use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use dotenvy::dotenv;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, FromRow, PgPool};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, services::ServeDir};
use uuid::Uuid;

const MAX_UPLOAD_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone)]
struct AppState {
    db: PgPool,
    paypal: PayPalConfig,
    http_client: Client,
    product_cache: Arc<RwLock<ProductCache>>,
}

struct ProductCache {
    products: Option<Vec<Product>>,
    updated_at: Instant,
}

#[derive(Clone)]
struct PayPalConfig {
    client_id: String,
    client_secret: String,
    base_url: String,
}

#[derive(Clone, Serialize, FromRow)]
struct Product {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    price_cents: i32,
    image_url: Option<String>,
    category: String,
}

#[derive(Serialize, FromRow)]
struct AdminProduct {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    price_cents: i32,
    image_url: Option<String>,
    category: String,
    active: bool,
}

#[derive(Deserialize)]
struct AdminProductRequest {
    slug: String,
    name: String,
    description: String,
    price_cents: i32,
    category: String,
    image_url: Option<String>,
    active: bool,
}

#[derive(Deserialize)]
struct UpdateProductActiveRequest {
    active: bool,
}

#[derive(Serialize)]
struct ProductImageUploadResponse {
    image_url: String,
}

#[derive(Deserialize)]
struct CreateOrderRequest {
    items: Vec<CreateOrderItem>,
}

#[derive(Serialize)]
struct OrderDetailResponse {
    id: Uuid,
    status: String,
    total_cents: i32,
    items: Vec<OrderItemResponse>,
}

#[derive(Serialize, FromRow)]
struct OrderItemResponse {
    id: Uuid,
    product_id: Uuid,
    product_name: String,
    unit_price_cents: i32,
    quantity: i32,
    line_total_cents: i32,
}

#[derive(Deserialize)]
struct CreateOrderItem {
    product_id: Uuid,
    quantity: i32,
}

struct OrderProductLine {
    product_id: Uuid,
    product_name: String,
    unit_price_cents: i32,
    quantity: i32,
    line_total_cents: i32,
}

#[derive(Serialize)]
struct CreateOrderResponse {
    order_id: Uuid,
    status: String,
    total_cents: i32,
}

#[derive(Deserialize)]
struct PayPalOrderRequest {
    order_id: Uuid,
}

#[derive(Deserialize)]
struct CapturePayPalOrderRequest {
    order_id: Uuid,
    paypal_order_id: String,
}

#[derive(Serialize)]
struct CreatePayPalOrderResponse {
    paypal_order_id: String,
}

#[derive(Serialize)]
struct CapturePayPalOrderResponse {
    paypal_order_id: String,
    paypal_status: String,
    order: OrderDetailResponse,
}

#[derive(Serialize, FromRow)]
struct AdminOrderSummary {
    id: Uuid,
    status: String,
    total_cents: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct AdminOrdersResponse {
    orders: Vec<AdminOrderSummary>,
}

#[derive(Deserialize)]
struct UpdateOrderStatusRequest {
    status: String,
}

#[derive(Deserialize)]
struct PayPalAccessTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct PayPalOrderResponse {
    id: String,
    status: String,
    purchase_units: Option<Vec<PayPalPurchaseUnit>>,
}

#[derive(Deserialize)]
struct PayPalPurchaseUnit {
    payments: Option<PayPalPayments>,
}

#[derive(Deserialize)]
struct PayPalPayments {
    captures: Vec<PayPalCapture>,
}

#[derive(Deserialize)]
struct PayPalCapture {
    status: String,
    amount: PayPalAmount,
    custom_id: Option<String>,
}

#[derive(Deserialize)]
struct PayPalAmount {
    currency_code: String,
    value: String,
}

async fn health() -> &'static str {
    "Charmaine Cat Studio API is running 🐱"
}

async fn get_paypal_access_token(state: &AppState) -> Result<String, StatusCode> {
    let response = state
        .http_client
        .post(format!("{}/v1/oauth2/token", state.paypal.base_url))
        .basic_auth(&state.paypal.client_id, Some(&state.paypal.client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await
        .map_err(|err| {
            eprintln!("Failed to request PayPal access token: {err}");
            StatusCode::BAD_GATEWAY
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        eprintln!("PayPal access token request failed ({status}): {body}");
        return Err(StatusCode::BAD_GATEWAY);
    }

    let token = response
        .json::<PayPalAccessTokenResponse>()
        .await
        .map_err(|err| {
            eprintln!("Failed to parse PayPal access token response: {err}");
            StatusCode::BAD_GATEWAY
        })?;

    Ok(token.access_token)
}

fn paypal_amount(total_cents: i32) -> String {
    format!("{}.{:02}", total_cents / 100, total_cents % 100)
}

fn is_admin_order_status(status: &str) -> bool {
    matches!(status, "shipped" | "completed" | "cancelled")
}

fn can_update_order_status(current_status: &str, next_status: &str) -> bool {
    if current_status == "cancelled" {
        return false;
    }

    match next_status {
        "cancelled" => true,
        "shipped" | "completed" => matches!(current_status, "paid" | "shipped"),
        _ => false,
    }
}

fn validate_product_payload(payload: &AdminProductRequest) -> Result<(), StatusCode> {
    if payload.slug.trim().is_empty()
        || payload.name.trim().is_empty()
        || payload.category.trim().is_empty()
        || payload.price_cents <= 0
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(())
}

async fn invalidate_product_cache(cache: &Arc<RwLock<ProductCache>>) {
    let mut cache = cache.write().await;
    cache.products = None;
    cache.updated_at = Instant::now();
    println!("[cache] products invalidated");
}

fn extension_for_image_content_type(content_type: &str) -> Option<&'static str> {
    match content_type.split(';').next()?.trim() {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

fn product_upload_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("uploads")
        .join("products")
}

async fn get_product_by_slug(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<Product>, StatusCode> {
    let product = sqlx::query_as::<_, Product>(
        r#"
        SELECT
            id,
            slug,
            name,
            description,
            price_cents,
            image_url,
            category
        FROM products
        WHERE slug = $1
          AND active = true
        "#,
    )
    .bind(slug)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to fetch product: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match product {
        Some(product) => Ok(Json(product)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<CreateOrderRequest>,
) -> Result<Json<CreateOrderResponse>, StatusCode> {
    if payload.items.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut tx = state.db.begin().await.map_err(|err| {
        eprintln!("Failed to begin transaction: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut lines = Vec::with_capacity(payload.items.len());

    for item in &payload.items {
        if item.quantity <= 0 {
            return Err(StatusCode::BAD_REQUEST);
        }

        let product = sqlx::query!(
            r#"
            SELECT id, name, price_cents
            FROM products
            WHERE id = $1
              AND active = true
            "#,
            item.product_id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| {
            eprintln!("Failed to fetch product for order: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let product = product.ok_or(StatusCode::BAD_REQUEST)?;
        let line_total_cents = product.price_cents * item.quantity;

        lines.push(OrderProductLine {
            product_id: product.id,
            product_name: product.name,
            unit_price_cents: product.price_cents,
            quantity: item.quantity,
            line_total_cents,
        });
    }

    let total_cents = lines.iter().map(|line| line.line_total_cents).sum::<i32>();

    let order = sqlx::query!(
        r#"
        INSERT INTO orders (status, total_cents)
        VALUES ('pending', $1)
        RETURNING id, status, total_cents
        "#,
        total_cents
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|err| {
        eprintln!("Failed to create order: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    for line in lines {
        sqlx::query!(
            r#"
            INSERT INTO order_items
            (
                order_id,
                product_id,
                product_name,
                unit_price_cents,
                quantity,
                line_total_cents
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            order.id,
            line.product_id,
            line.product_name,
            line.unit_price_cents,
            line.quantity,
            line.line_total_cents
        )
        .execute(&mut *tx)
        .await
        .map_err(|err| {
            eprintln!("Failed to create order item: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    tx.commit().await.map_err(|err| {
        eprintln!("Failed to commit order transaction: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(CreateOrderResponse {
        order_id: order.id,
        status: order.status,
        total_cents: order.total_cents,
    }))
}

async fn get_order_by_id(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
) -> Result<Json<OrderDetailResponse>, StatusCode> {
    Ok(Json(fetch_order_detail(&state.db, order_id).await?))
}

async fn fetch_order_detail(
    db: &PgPool,
    order_id: Uuid,
) -> Result<OrderDetailResponse, StatusCode> {
    let order = sqlx::query!(
        r#"
        SELECT id, status, total_cents
        FROM orders
        WHERE id = $1
        "#,
        order_id
    )
    .fetch_optional(db)
    .await
    .map_err(|err| {
        eprintln!("Failed to fetch order: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let order = order.ok_or(StatusCode::NOT_FOUND)?;

    let items = sqlx::query_as::<_, OrderItemResponse>(
        r#"
        SELECT
            id,
            product_id,
            product_name,
            unit_price_cents,
            quantity,
            line_total_cents
        FROM order_items
        WHERE order_id = $1
        ORDER BY id ASC
        "#,
    )
    .bind(order_id)
    .fetch_all(db)
    .await
    .map_err(|err| {
        eprintln!("Failed to fetch order items: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(OrderDetailResponse {
        id: order.id,
        status: order.status,
        total_cents: order.total_cents,
        items,
    })
}

async fn create_paypal_order(
    State(state): State<AppState>,
    Json(payload): Json<PayPalOrderRequest>,
) -> Result<Json<CreatePayPalOrderResponse>, StatusCode> {
    let order = sqlx::query!(
        r#"
        SELECT status, total_cents
        FROM orders
        WHERE id = $1
        "#,
        payload.order_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to fetch order for PayPal: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    if order.status != "pending" {
        return Err(StatusCode::CONFLICT);
    }

    if order.total_cents <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let access_token = get_paypal_access_token(&state).await?;
    let response = state
        .http_client
        .post(format!("{}/v2/checkout/orders", state.paypal.base_url))
        .bearer_auth(access_token)
        .json(&serde_json::json!({
            "intent": "CAPTURE",
            "purchase_units": [{
                "custom_id": payload.order_id.to_string(),
                "description": format!("Charmaine Cat Studio order {}", payload.order_id),
                "amount": {
                    "currency_code": "CAD",
                    "value": paypal_amount(order.total_cents)
                }
            }]
        }))
        .send()
        .await
        .map_err(|err| {
            eprintln!("Failed to create PayPal order: {err}");
            StatusCode::BAD_GATEWAY
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        eprintln!("PayPal order creation failed ({status}): {body}");
        return Err(StatusCode::BAD_GATEWAY);
    }

    let status = response.status();
    let body = response.text().await.map_err(|err| {
        eprintln!("Failed to read PayPal order response body: {err}");
        StatusCode::BAD_GATEWAY
    })?;

    if !status.is_success() {
        eprintln!("PayPal order creation failed ({status}): {body}");
        return Err(StatusCode::BAD_GATEWAY);
    }

    let paypal_order: PayPalOrderResponse = serde_json::from_str(&body).map_err(|err| {
        eprintln!("Failed to parse PayPal order response: {err}; body={body}");
        StatusCode::BAD_GATEWAY
    })?;

    Ok(Json(CreatePayPalOrderResponse {
        paypal_order_id: paypal_order.id,
    }))
}

async fn capture_paypal_order(
    State(state): State<AppState>,
    Json(payload): Json<CapturePayPalOrderRequest>,
) -> Result<Json<CapturePayPalOrderResponse>, StatusCode> {
    let order = sqlx::query!(
        r#"
        SELECT status, total_cents
        FROM orders
        WHERE id = $1
        "#,
        payload.order_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to fetch order for PayPal capture: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    if order.status != "pending" {
        return Err(StatusCode::CONFLICT);
    }

    let access_token = get_paypal_access_token(&state).await?;
    let response = state
        .http_client
        .post(format!(
            "{}/v2/checkout/orders/{}/capture",
            state.paypal.base_url, payload.paypal_order_id
        ))
        .bearer_auth(access_token)
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .map_err(|err| {
            eprintln!("Failed to capture PayPal order: {err}");
            StatusCode::BAD_GATEWAY
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        eprintln!("PayPal order capture failed ({status}): {body}");
        return Err(StatusCode::BAD_GATEWAY);
    }

    let status = response.status();
    let body = response.text().await.map_err(|err| {
        eprintln!("Failed to read PayPal capture response body: {err}");
        StatusCode::BAD_GATEWAY
    })?;

    if !status.is_success() {
        eprintln!("PayPal order capture failed ({status}): {body}");
        return Err(StatusCode::BAD_GATEWAY);
    }

    eprintln!("PayPal capture response body: {body}");

    let paypal_order: PayPalOrderResponse = serde_json::from_str(&body).map_err(|err| {
        eprintln!("Failed to parse PayPal capture response: {err}; body={body}");
        StatusCode::BAD_GATEWAY
    })?;

    let expected_order_id = payload.order_id.to_string();
    let expected_amount = paypal_amount(order.total_cents);
    let valid_capture = paypal_order.status == "COMPLETED"
        && paypal_order
            .purchase_units
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .any(|unit| {
                unit.payments.as_ref().is_some_and(|payments| {
                    payments.captures.iter().any(|capture| {
                        capture.status == "COMPLETED"
                            && capture.custom_id.as_deref() == Some(expected_order_id.as_str())
                            && capture.amount.currency_code == "CAD"
                            && capture.amount.value == expected_amount
                    })
                })
            });

    if !valid_capture {
        eprintln!(
            "PayPal capture verification failed for local order {}",
            payload.order_id
        );
        return Err(StatusCode::BAD_GATEWAY);
    }

    let update_result = sqlx::query(
        r#"
        UPDATE orders
        SET status = 'paid',
            updated_at = NOW()
        WHERE id = $1
          AND status = 'pending'
        "#,
    )
    .bind(payload.order_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to mark order as paid: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if update_result.rows_affected() != 1 {
        return Err(StatusCode::CONFLICT);
    }

    let updated_order = fetch_order_detail(&state.db, payload.order_id).await?;

    Ok(Json(CapturePayPalOrderResponse {
        paypal_order_id: paypal_order.id,
        paypal_status: paypal_order.status,
        order: updated_order,
    }))
}

async fn list_products(State(state): State<AppState>) -> Result<Json<Vec<Product>>, StatusCode> {
    {
        let cache = state.product_cache.read().await;
        if let Some(products) = &cache.products {
            println!("[cache] products hit");
            return Ok(Json(products.clone()));
        }
    }

    println!("[cache] products miss");

    let products = sqlx::query_as::<_, Product>(
        r#"
        SELECT
            id,
            slug,
            name,
            description,
            price_cents,
            image_url,
            category
        FROM products
        WHERE active = true
        ORDER BY created_at ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to fetch products: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    {
        let mut cache = state.product_cache.write().await;
        cache.products = Some(products.clone());
        cache.updated_at = Instant::now();
    }

    Ok(Json(products))
}

async fn list_admin_products(
    State(state): State<AppState>,
) -> Result<Json<Vec<AdminProduct>>, StatusCode> {
    let products = sqlx::query_as::<_, AdminProduct>(
        r#"
        SELECT
            id,
            slug,
            name,
            description,
            price_cents,
            image_url,
            category,
            active
        FROM products
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to fetch admin products: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(products))
}

async fn create_admin_product(
    State(state): State<AppState>,
    Json(payload): Json<AdminProductRequest>,
) -> Result<Json<AdminProduct>, StatusCode> {
    validate_product_payload(&payload)?;

    let product = sqlx::query_as::<_, AdminProduct>(
        r#"
        INSERT INTO products
        (
            slug,
            name,
            description,
            price_cents,
            image_url,
            category,
            active
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING
            id,
            slug,
            name,
            description,
            price_cents,
            image_url,
            category,
            active
        "#,
    )
    .bind(payload.slug.trim())
    .bind(payload.name.trim())
    .bind(payload.description)
    .bind(payload.price_cents)
    .bind(payload.image_url.and_then(|url| {
        let trimmed = url.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    }))
    .bind(payload.category.trim())
    .bind(payload.active)
    .fetch_one(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to create product: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    invalidate_product_cache(&state.product_cache).await;

    Ok(Json(product))
}

async fn update_admin_product(
    State(state): State<AppState>,
    Path(product_id): Path<Uuid>,
    Json(payload): Json<AdminProductRequest>,
) -> Result<Json<AdminProduct>, StatusCode> {
    validate_product_payload(&payload)?;

    let product = sqlx::query_as::<_, AdminProduct>(
        r#"
        UPDATE products
        SET slug = $1,
            name = $2,
            description = $3,
            price_cents = $4,
            image_url = $5,
            category = $6,
            active = $7
        WHERE id = $8
        RETURNING
            id,
            slug,
            name,
            description,
            price_cents,
            image_url,
            category,
            active
        "#,
    )
    .bind(payload.slug.trim())
    .bind(payload.name.trim())
    .bind(payload.description)
    .bind(payload.price_cents)
    .bind(payload.image_url.and_then(|url| {
        let trimmed = url.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    }))
    .bind(payload.category.trim())
    .bind(payload.active)
    .bind(product_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to update product: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match product {
        Some(product) => {
            invalidate_product_cache(&state.product_cache).await;
            Ok(Json(product))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn update_admin_product_active(
    State(state): State<AppState>,
    Path(product_id): Path<Uuid>,
    Json(payload): Json<UpdateProductActiveRequest>,
) -> Result<Json<AdminProduct>, StatusCode> {
    let product = sqlx::query_as::<_, AdminProduct>(
        r#"
        UPDATE products
        SET active = $1
        WHERE id = $2
        RETURNING
            id,
            slug,
            name,
            description,
            price_cents,
            image_url,
            category,
            active
        "#,
    )
    .bind(payload.active)
    .bind(product_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to update product active status: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match product {
        Some(product) => {
            invalidate_product_cache(&state.product_cache).await;
            Ok(Json(product))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn delete_admin_product(
    State(state): State<AppState>,
    Path(product_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let result = sqlx::query(
        r#"
        DELETE FROM products
        WHERE id = $1
        "#,
    )
    .bind(product_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to delete product: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    invalidate_product_cache(&state.product_cache).await;

    Ok(StatusCode::NO_CONTENT)
}

async fn upload_admin_product_image(
    mut multipart: Multipart,
) -> Result<Json<ProductImageUploadResponse>, StatusCode> {
    while let Some(field) = multipart.next_field().await.map_err(|err| {
        eprintln!("Failed to read upload field: {err}");
        StatusCode::BAD_REQUEST
    })? {
        if field.name() != Some("file") {
            continue;
        }

        let extension = field
            .content_type()
            .and_then(extension_for_image_content_type)
            .ok_or(StatusCode::BAD_REQUEST)?;

        let mut field = field;
        let mut bytes = Vec::new();

        while let Some(chunk) = field.chunk().await.map_err(|err| {
            eprintln!("Failed to read uploaded image bytes: {err}");
            StatusCode::BAD_REQUEST
        })? {
            if bytes.len() + chunk.len() > MAX_UPLOAD_BYTES {
                return Err(StatusCode::BAD_REQUEST);
            }

            bytes.extend_from_slice(&chunk);
        }

        if bytes.is_empty() || bytes.len() > MAX_UPLOAD_BYTES {
            return Err(StatusCode::BAD_REQUEST);
        }

        let upload_dir = product_upload_dir();

        tokio::fs::create_dir_all(&upload_dir)
            .await
            .map_err(|err| {
                eprintln!("Failed to create product upload directory: {err}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let filename = format!("{}.{}", Uuid::new_v4(), extension);
        let file_path = upload_dir.join(&filename);

        tokio::fs::write(file_path, bytes).await.map_err(|err| {
            eprintln!("Failed to save product image upload: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        return Ok(Json(ProductImageUploadResponse {
            image_url: format!("/uploads/products/{filename}"),
        }));
    }

    Err(StatusCode::BAD_REQUEST)
}

async fn list_admin_orders(
    State(state): State<AppState>,
) -> Result<Json<AdminOrdersResponse>, StatusCode> {
    let orders = sqlx::query_as::<_, AdminOrderSummary>(
        r#"
        SELECT
            id,
            status,
            total_cents,
            created_at,
            updated_at
        FROM orders
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to fetch admin orders: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(AdminOrdersResponse { orders }))
}

async fn update_admin_order_status(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    Json(payload): Json<UpdateOrderStatusRequest>,
) -> Result<Json<AdminOrderSummary>, StatusCode> {
    if !is_admin_order_status(&payload.status) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let order = sqlx::query!(
        r#"
        SELECT status
        FROM orders
        WHERE id = $1
        "#,
        order_id
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to fetch order for admin status update: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    if !can_update_order_status(&order.status, &payload.status) {
        return Err(StatusCode::CONFLICT);
    }

    let updated_order = sqlx::query_as::<_, AdminOrderSummary>(
        r#"
        UPDATE orders
        SET status = $1,
            updated_at = NOW()
        WHERE id = $2
          AND status = $3
        RETURNING
            id,
            status,
            total_cents,
            created_at,
            updated_at
        "#,
    )
    .bind(payload.status)
    .bind(order_id)
    .bind(order.status)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to update order status: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match updated_order {
        Some(order) => Ok(Json(order)),
        None => Err(StatusCode::CONFLICT),
    }
}

fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("migrations")
}

async fn run_migrations(db: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(db)
    .await?;

    let mut entries = tokio::fs::read_dir(migrations_dir()).await?;
    let mut files = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) == Some("sql") {
            files.push(path);
        }
    }

    files.sort();

    for path in files {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid migration filename")?
            .to_string();

        let already_applied = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = $1",
        )
        .bind(&filename)
        .fetch_one(db)
        .await?;

        if already_applied > 0 {
            println!("[migrate] skipped {filename}");
            continue;
        }

        let sql = tokio::fs::read_to_string(&path).await?;
        let mut tx = db.begin().await?;

        sqlx::raw_sql(&sql).execute(&mut *tx).await?;

        sqlx::query("INSERT INTO schema_migrations (version) VALUES ($1)")
            .bind(&filename)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        println!("[migrate] applied {filename}");
    }

    println!("[migrate] done");

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let db_max_connections: u32 = std::env::var("DB_MAX_CONNECTIONS")
        .unwrap_or_else(|_| "5".to_string())
        .parse()
        .expect("DB_MAX_CONNECTIONS must be a positive integer");

    let db = PgPoolOptions::new()
        .max_connections(db_max_connections)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    if std::env::args().nth(1).as_deref() == Some("migrate") {
        if let Err(err) = run_migrations(&db).await {
            eprintln!("[migrate] failed: {err}");
            std::process::exit(1);
        }

        return;
    }

    let paypal = PayPalConfig {
        client_id: std::env::var("PAYPAL_CLIENT_ID").expect("PAYPAL_CLIENT_ID must be set"),
        client_secret: std::env::var("PAYPAL_CLIENT_SECRET")
            .expect("PAYPAL_CLIENT_SECRET must be set"),
        base_url: std::env::var("PAYPAL_BASE_URL")
            .unwrap_or_else(|_| "https://api-m.sandbox.paypal.com".to_string())
            .trim_end_matches('/')
            .to_string(),
    };

    let state = AppState {
        db,
        paypal,
        http_client: Client::new(),
        product_cache: Arc::new(RwLock::new(ProductCache {
            products: None,
            updated_at: Instant::now(),
        })),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/products", get(list_products))
        .route("/products/{slug}", get(get_product_by_slug))
        .route("/orders", post(create_order))
        .route("/orders/{id}", get(get_order_by_id))
        .route("/payments/paypal/create-order", post(create_paypal_order))
        .route("/payments/paypal/capture-order", post(capture_paypal_order))
        .route("/admin/orders", get(list_admin_orders))
        .route(
            "/admin/orders/{id}/status",
            patch(update_admin_order_status),
        )
        .route(
            "/admin/products",
            get(list_admin_products).post(create_admin_product),
        )
        .route(
            "/admin/products/{id}",
            patch(update_admin_product).delete(delete_admin_product),
        )
        .route(
            "/admin/products/{id}/active",
            patch(update_admin_product_active),
        )
        .route(
            "/admin/uploads/product-image",
            post(upload_admin_product_image).layer(DefaultBodyLimit::disable()),
        )
        .nest_service("/uploads/products", ServeDir::new(product_upload_dir()))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind server");

    println!("🐱 Charmaine Cat Studio API running on http://localhost:8080");

    axum::serve(listener, app).await.expect("server failed");
}

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use dotenvy::dotenv;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, FromRow, PgPool};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: PgPool,
    paypal: PayPalConfig,
    http_client: Client,
}

#[derive(Clone)]
struct PayPalConfig {
    client_id: String,
    client_secret: String,
    base_url: String,
}

#[derive(Serialize, FromRow)]
struct Product {
    id: Uuid,
    slug: String,
    name: String,
    description: String,
    price_cents: i32,
    image_url: Option<String>,
    category: String,
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

    let mut total_cents = 0;

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

        total_cents += product.price_cents * item.quantity;
    }

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

    for item in &payload.items {
        let product = sqlx::query!(
            r#"
            SELECT id, name, price_cents
            FROM products
            WHERE id = $1
              AND active = true
            "#,
            item.product_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| {
            eprintln!("Failed to fetch product for order item: {err}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let line_total_cents = product.price_cents * item.quantity;

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
            product.id,
            product.name,
            product.price_cents,
            item.quantity,
            line_total_cents
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

    let update_result = sqlx::query!(
        r#"
        UPDATE orders
        SET status = 'paid'
        WHERE id = $1
          AND status = 'pending'
        "#,
        payload.order_id
    )
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

    Ok(Json(products))
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let paypal = PayPalConfig {
        client_id: std::env::var("PAYPAL_CLIENT_ID").expect("PAYPAL_CLIENT_ID must be set"),
        client_secret: std::env::var("PAYPAL_CLIENT_SECRET")
            .expect("PAYPAL_CLIENT_SECRET must be set"),
        base_url: std::env::var("PAYPAL_BASE_URL")
            .unwrap_or_else(|_| "https://api-m.sandbox.paypal.com".to_string())
            .trim_end_matches('/')
            .to_string(),
    };

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    let state = AppState {
        db,
        paypal,
        http_client: Client::new(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/products", get(list_products))
        .route("/products/{slug}", get(get_product_by_slug))
        .route("/orders", post(create_order))
        .route("/orders/{id}", get(get_order_by_id))
        .route("/payments/paypal/create-order", post(create_paypal_order))
        .route("/payments/paypal/capture-order", post(capture_paypal_order))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind server");

    println!("🐱 Charmaine Cat Studio API running on http://localhost:8080");

    axum::serve(listener, app).await.expect("server failed");
}

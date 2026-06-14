use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, FromRow, PgPool};
use tower_http::cors::CorsLayer;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: PgPool,
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

async fn health() -> &'static str {
    "Charmaine Cat Studio API is running 🐱"
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
    let order = sqlx::query!(
        r#"
        SELECT id, status, total_cents
        FROM orders
        WHERE id = $1
        "#,
        order_id
    )
    .fetch_optional(&state.db)
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
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        eprintln!("Failed to fetch order items: {err}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(OrderDetailResponse {
        id: order.id,
        status: order.status,
        total_cents: order.total_cents,
        items,
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

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    let state = AppState { db };

    let app = Router::new()
        .route("/health", get(health))
        .route("/products", get(list_products))
        .route("/products/{slug}", get(get_product_by_slug))
        .route("/orders", post(create_order))
        .route("/orders/{id}", get(get_order_by_id))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind server");

    println!("🐱 Charmaine Cat Studio API running on http://localhost:8080");

    axum::serve(listener, app).await.expect("server failed");
}

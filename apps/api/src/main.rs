use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use dotenvy::dotenv;
use serde::Serialize;
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

async fn health() -> &'static str {
    "Charmaine Cat Studio API is running 🐱"
}

async fn list_products(
    State(state): State<AppState>,
) -> Result<Json<Vec<Product>>, StatusCode> {
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

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    let state = AppState { db };

    let app = Router::new()
        .route("/health", get(health))
        .route("/products", get(list_products))
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind server");

    println!("🐱 Charmaine Cat Studio API running on http://localhost:8080");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}
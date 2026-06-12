use axum::{
    routing::get,
    Json, Router,
};
use serde::Serialize;
use tower_http::cors::CorsLayer;

#[derive(Serialize)]
struct Product {
    id: String,
    name: String,
    description: String,
    price_cents: i32,
    image_url: String,
    category: String,
}

async fn health() -> &'static str {
    "Charmaine Cat Studio API is running 🐱"
}

async fn list_products() -> Json<Vec<Product>> {
    Json(vec![
        Product {
            id: "mango-mission-shirt".to_string(),
            name: "Mango Mission T-Shirt".to_string(),
            description: "A cute Charmaine Cat design for mango lovers and coding cats.".to_string(),
            price_cents: 2500,
            image_url: "/products/mango-mission.png".to_string(),
            category: "Merchandise".to_string(),
        },
        Product {
            id: "rust-dev-cat".to_string(),
            name: "Rust Developer Cat".to_string(),
            description: "For developers who like safe code, fast APIs, and small cute cats.".to_string(),
            price_cents: 3000,
            image_url: "/products/rust-dev-cat.png".to_string(),
            category: "Merchandise".to_string(),
        },
        Product {
            id: "buy-charmaine-a-mango".to_string(),
            name: "Buy Charmaine Cat a Mango".to_string(),
            description: "A virtual support item for Charmaine Cat Studio.".to_string(),
            price_cents: 500,
            image_url: "/products/mango-support.png".to_string(),
            category: "Support".to_string(),
        },
    ])
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health))
        .route("/products", get(list_products))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind server");

    println!("🐱 Charmaine Cat Studio API running on http://localhost:8080");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}
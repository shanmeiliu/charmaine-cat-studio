use super::MetricsRegistry;
use axum::{
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::Response,
};
use std::{sync::Arc, time::Instant};

pub async fn record_metrics(
    State(metrics): State<Arc<MetricsRegistry>>,
    request: Request,
    next: Next,
) -> Response {
    let started_at = Instant::now();
    let method = request.method().as_str().to_string();
    let endpoint = request
        .extensions()
        .get::<MatchedPath>()
        .map(|matched_path| matched_path.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());

    let response = next.run(request).await;
    let elapsed_micros = started_at.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;

    metrics.record_request(
        &method,
        &endpoint,
        response.status().as_u16(),
        elapsed_micros,
    );

    response
}

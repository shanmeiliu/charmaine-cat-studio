use super::Trace;
use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

pub async fn trace_requests(mut request: Request, next: Next) -> Response {
    let trace = Trace::new(
        Uuid::new_v4().to_string(),
        request.method().as_str().to_string(),
        request.uri().path().to_string(),
    );
    let request_id = trace.request_id();

    request.extensions_mut().insert(trace.clone());

    let mut response = next.run(request).await;
    let status = response.status().as_u16();

    if let Ok(header_value) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(REQUEST_ID_HEADER, header_value);
    }

    trace.log_completed(status);

    response
}

use axum::{
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use subtle::ConstantTimeEq;

use super::McpShared;

/// Bearer token + method gate for the MCP endpoint. Host/Origin/body-size
/// validation is configured on `StreamableHttpServerConfig`; this middleware
/// only handles the credential and the POST-only transport.
pub async fn auth_middleware(
    State(shared): State<Arc<McpShared>>,
    request: Request,
    next: Next,
) -> Response {
    if request.method() != Method::POST {
        return (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "POST")]).into_response();
    }

    let expected = shared
        .token
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let provided = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));

    let authorized = provided
        .map(|value| bool::from(value.as_bytes().ct_eq(expected.as_bytes())))
        .unwrap_or(false);

    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
        )
            .into_response();
    }

    next.run(request).await
}

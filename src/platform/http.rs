//! What wraps every request: a span naming it, a log line closing it, and the headers a page
//! outside a panel carries. Panels set their own stricter headers on their responses.

use std::time::Instant;

use topcoat::{
    Result,
    context::{Cx, try_context},
    router::{
        Body, HeaderName, HeaderValue, Next,
        header::{
            REFERRER_POLICY, STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
        },
        layer, request,
        response::{IntoResponse, Response},
    },
};
use tracing::{Instrument, info_span};

/// The request id a proxy may have set, echoed back and carried on every log line.
const REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Whether the app answers over HTTPS, which turns HSTS on.
#[derive(Clone, Copy)]
pub struct Https(pub bool);

#[layer("/")]
async fn observe(cx: &Cx, body: Body, next: Next<'_>) -> Result<Response> {
    let id = request::headers(cx)
        .get(&REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.len() <= 64 && value.is_ascii())
        .map_or_else(|| uuid::Uuid::now_v7().to_string(), ToString::to_string);
    let path = request::uri(cx).path();
    let method = request::method(cx);
    let span = info_span!("request", %method, path, request_id = %id);
    let started = Instant::now();
    let response = next.run(cx, body).instrument(span.clone()).await;
    let mut response = match response {
        Ok(response) => response,
        Err(error) => error.into_response(cx)?,
    };
    let elapsed_ms = started.elapsed().as_millis();
    let status = response.status().as_u16();
    span.in_scope(|| tracing::info!(status, elapsed_ms, "served"));
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&id) {
        headers.insert(REQUEST_ID, value);
    }
    // `insert` only where absent: a panel's own, stricter value stands.
    headers
        .entry(X_CONTENT_TYPE_OPTIONS)
        .or_insert(HeaderValue::from_static("nosniff"));
    headers
        .entry(X_FRAME_OPTIONS)
        .or_insert(HeaderValue::from_static("DENY"));
    headers
        .entry(REFERRER_POLICY)
        .or_insert(HeaderValue::from_static("strict-origin-when-cross-origin"));
    if try_context::<Https>(cx).is_some_and(|https| https.0) {
        headers
            .entry(STRICT_TRANSPORT_SECURITY)
            .or_insert(HeaderValue::from_static(
                "max-age=63072000; includeSubDomains",
            ));
    }
    Ok(response)
}

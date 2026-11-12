//! The product's name, marks and themes, the marks served by the app itself.

use tablo::{Brand, Theme};
use topcoat::{
    Result,
    context::Cx,
    router::{
        HeaderMap, HeaderValue,
        header::{CACHE_CONTROL, CONTENT_TYPE},
        route,
    },
};

/// The product's name, on every panel, page title and mail.
pub const PRODUCT_NAME: &str = "Tablo SaaS";

const LOGO_URL: &str = "/brand/logo.svg";
const DARK_LOGO_URL: &str = "/brand/logo-dark.svg";

/// The product's brand in `theme`: Iris where customers work and on the marketing site, Graphite
/// where staff do, so neither is mistaken for the other.
pub(super) fn brand(theme: Theme) -> Brand {
    Brand::new(PRODUCT_NAME)
        .logo(LOGO_URL)
        .dark_logo(DARK_LOGO_URL)
        .favicon(LOGO_URL)
        .theme(theme)
}

/// `GET /brand/logo.svg`.
#[route(GET "/brand/logo.svg")]
async fn logo(cx: &Cx) -> Result<(HeaderMap, &'static str)> {
    Ok(svg(include_str!("../../assets/brand/logo.svg")))
}

/// `GET /brand/logo-dark.svg`.
#[route(GET "/brand/logo-dark.svg")]
async fn dark_logo(cx: &Cx) -> Result<(HeaderMap, &'static str)> {
    Ok(svg(include_str!("../../assets/brand/logo-dark.svg")))
}

fn svg(body: &'static str) -> (HeaderMap, &'static str) {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"));
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=86400"),
    );
    (headers, body)
}

//! The product's name, mark and themes, the mark served by the app itself.

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

/// The URL of Tablo's mark: the logo of every panel, its favicon, and the mark on the site.
pub const LOGO_URL: &str = "/brand/logo.svg";

/// The product's brand on every panel, in Ledger: the theme of the public site (`ledger.css`), of
/// Tablo's own site and of its demo, so the product reads as one from the site to the panels. The
/// mark carries Ledger's ink and red.
pub(super) fn brand() -> Brand {
    Brand::new(PRODUCT_NAME)
        .logo(LOGO_URL)
        .favicon(LOGO_URL)
        .theme(Theme::Ledger)
}

/// `GET /brand/logo.svg`.
#[route(GET "/brand/logo.svg")]
async fn logo(cx: &Cx) -> Result<(HeaderMap, &'static str)> {
    Ok(svg(include_str!("../../assets/brand/logo.svg")))
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

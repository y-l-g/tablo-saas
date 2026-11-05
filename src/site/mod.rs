//! The public website, outside every panel. It borrows the `app` panel's document and catalogs, so
//! it speaks the visitor's language and shares its stylesheet.

use tablo::PanelHandle;
use topcoat::{
    Result,
    context::{Cx, context},
    router::{HeaderMap, HeaderValue, header::CONTENT_TYPE, page, route},
    view::{BoxView, View, ViewExt, view},
};

use crate::billing::{CURRENCY, Plan, money, plan_name};
use crate::panels::PRODUCT_NAME;
use crate::platform::config::Settings;

/// The mounted panel whose document and catalogs the site borrows.
#[derive(Clone)]
pub struct SitePanel(pub PanelHandle);

/// The pages a search engine should know, as paths.
const PUBLIC_PATHS: [&str; 4] = ["/", "/pricing", "/terms", "/privacy"];

/// The header and footer every public page shares.
fn frame<'a>(outer: &'a Cx, cx: &Cx, content: BoxView<'a>) -> BoxView<'a> {
    let pricing_label = tablo::t!(cx, "site-pricing");
    let sign_in = tablo::t!(cx, "site-sign-in");
    let sign_up = tablo::t!(cx, "site-sign-up");
    let terms_label = tablo::t!(cx, "site-terms");
    let privacy_label = tablo::t!(cx, "site-privacy");
    view! {
        outer =>
        <div class="mx-auto flex min-h-screen max-w-5xl flex-col gap-12 px-6 py-8">
            <header class="flex items-center justify-between gap-4">
                <a href="/" class="font-semibold">(PRODUCT_NAME)</a>
                <nav class="flex items-center gap-4 text-sm">
                    <a href="/pricing">(pricing_label)</a>
                    <a href="/app/login">(sign_in)</a>
                    <a href="/app/register" class="rounded-lg bg-primary px-3 py-1.5 text-primary-foreground">
                        (sign_up)
                    </a>
                </nav>
            </header>
            <main class="flex flex-1 flex-col gap-10">(content)</main>
            <footer class="flex gap-4 text-sm text-muted-foreground">
                <a href="/terms">(terms_label)</a>
                <a href="/privacy">(privacy_label)</a>
            </footer>
        </div>
    }
    .boxed()
}

/// `GET /`.
#[page("/")]
async fn home(cx: &Cx) -> Result<impl View> {
    let SitePanel(panel) = context::<SitePanel>(cx);
    let entered = panel.enter(cx);
    let title = PRODUCT_NAME.to_string();
    let headline = tablo::t!(&entered, "site-headline");
    let lead = tablo::t!(&entered, "site-lead");
    let start = tablo::t!(&entered, "site-start");
    let features = [
        tablo::t!(&entered, "site-feature-teams"),
        tablo::t!(&entered, "site-feature-billing"),
        tablo::t!(&entered, "site-feature-security"),
    ];
    let content = view! {
        cx =>
        <section class="flex flex-col items-start gap-4 py-12">
            <h1 class="max-w-2xl text-4xl font-bold tracking-tight">(headline)</h1>
            <p class="max-w-xl text-lg text-muted-foreground">(lead)</p>
            <a href="/app/register" class="rounded-lg bg-primary px-4 py-2 text-primary-foreground">
                (start)
            </a>
        </section>
        <section class="grid gap-6 md:grid-cols-3">
            for feature in features {
                <p class="rounded-lg border border-border p-4 text-sm">(feature)</p>
            }
        </section>
    }
    .boxed();
    panel
        .document(cx, title, frame(cx, &entered, content))
        .await
}

/// `GET /pricing`, read from the plan catalog, so the page cannot disagree with what is sold.
#[page("/pricing")]
async fn pricing(cx: &Cx) -> Result<impl View> {
    let SitePanel(panel) = context::<SitePanel>(cx);
    let entered = panel.enter(cx);
    let title = tablo::t!(&entered, "site-pricing");
    let heading = title.clone();
    let start = tablo::t!(&entered, "site-start");
    let plans: Vec<(String, String, String, String)> = Plan::ALL
        .into_iter()
        .map(|plan| {
            let price = if plan.monthly_amount == 0 {
                tablo::t!(&entered, "site-price-free")
            } else {
                tablo::t!(
                    &entered,
                    "billing-per-seat",
                    price = money(plan.monthly_amount, CURRENCY)
                )
            };
            let limit = |limit: Option<u32>| {
                limit.map_or_else(
                    || tablo::t!(&entered, "site-unlimited"),
                    |limit| limit.to_string(),
                )
            };
            (
                plan_name(&entered, plan.id),
                price,
                tablo::t!(&entered, "site-plan-seats", seats = limit(plan.seat_limit)),
                tablo::t!(
                    &entered,
                    "site-plan-projects",
                    projects = limit(plan.project_limit)
                ),
            )
        })
        .collect();
    let content = view! {
        cx =>
        <h1 class="text-3xl font-bold tracking-tight">(heading)</h1>
        <div class="grid gap-6 md:grid-cols-3">
            for (name, price, seats, projects) in plans {
                <article class="flex flex-col gap-2 rounded-lg border border-border p-5">
                    <h2 class="text-xl font-semibold">(name)</h2>
                    <p class="text-muted-foreground">(price)</p>
                    <p class="text-sm">(seats)</p>
                    <p class="text-sm">(projects)</p>
                </article>
            }
        </div>
        <a href="/app/register" class="self-start rounded-lg bg-primary px-4 py-2 text-primary-foreground">
            (start)
        </a>
    }
    .boxed();
    panel
        .document(cx, title, frame(cx, &entered, content))
        .await
}

/// A page of prose: a title and a sample the owner must replace before launch.
async fn legal(cx: &Cx, title_id: &'static str, body_id: &'static str) -> Result<impl View> {
    let SitePanel(panel) = context::<SitePanel>(cx);
    let entered = panel.enter(cx);
    let title = tablo::t!(&entered, title_id);
    let heading = title.clone();
    let body = tablo::t!(&entered, body_id);
    let content = view! {
        cx =>
        <h1 class="text-3xl font-bold tracking-tight">(heading)</h1>
        <p class="max-w-2xl">(body)</p>
    }
    .boxed();
    panel
        .document(cx, title, frame(cx, &entered, content))
        .await
}

/// `GET /terms`.
#[page("/terms")]
async fn terms(cx: &Cx) -> Result<impl View> {
    legal(cx, "site-terms", "site-terms-body").await
}

/// `GET /privacy`.
#[page("/privacy")]
async fn privacy(cx: &Cx) -> Result<impl View> {
    legal(cx, "site-privacy", "site-privacy-body").await
}

fn typed(kind: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(kind));
    headers
}

/// `GET /robots.txt`: the site is public and the panels are not.
#[route(GET "/robots.txt")]
async fn robots(cx: &Cx) -> Result<(HeaderMap, String)> {
    let base = &context::<Settings>(cx).base_url;
    Ok((
        typed("text/plain; charset=utf-8"),
        format!("User-agent: *\nDisallow: /app\nDisallow: /admin\nSitemap: {base}/sitemap.xml\n"),
    ))
}

/// `GET /sitemap.xml`.
#[route(GET "/sitemap.xml")]
async fn sitemap(cx: &Cx) -> Result<(HeaderMap, String)> {
    let base = &context::<Settings>(cx).base_url;
    let urls = PUBLIC_PATHS.map(|path| format!("<url><loc>{base}{path}</loc></url>"));
    let urls = urls.concat();
    Ok((
        typed("application/xml"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{urls}</urlset>"#
        ),
    ))
}

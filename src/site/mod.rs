//! The public website, outside every panel. It borrows the `app` panel's document and catalogs, so
//! it speaks the visitor's language and follows the theme picked in the app.
//!
//! It wears Ledger, the identity Tablo's site and demo share (`ledger.css`), and presents the
//! template as it ships. A product built on the template replaces the copy under `## Website` in
//! `locales/`, and `ledger.css` with its own brand if it wants one.

use tablo::PanelHandle;
use topcoat::{
    Result,
    context::{Cx, context},
    font::{Font, fontsource::fontsource_font},
    router::{HeaderMap, HeaderValue, header::CONTENT_TYPE, page, route},
    view::{BoxView, View, ViewExt, component, view},
};

use crate::billing::{CURRENCY, Feature, Plan, money, plan_name};
use crate::panels::PRODUCT_NAME;
use crate::platform::config::Settings;

/// The mounted panel whose document and catalogs the site borrows.
#[derive(Clone)]
pub struct SitePanel {
    /// The `app` panel, whose document wraps every public page.
    pub panel: PanelHandle,
    /// Whether the router serves assets, without which the site links no font.
    pub styled: bool,
}

/// The pages a search engine should know, as paths.
const PUBLIC_PATHS: [&str; 4] = ["/", "/pricing", "/terms", "/privacy"];

const SOURCE: &str = "https://github.com/y-l-g/tablo-saas";
const TREE: &str = "https://github.com/y-l-g/tablo-saas/tree/master/";
const TABLO_SITE: &str = "https://y-l.fr/tablo/";
const GUIDE: &str = "https://y-l.fr/tablo/nightly/guide/";
const EXAMPLES: &str = "https://y-l.fr/tablo/examples/";
const DEMO: &str = "https://demo.tablo.y-l.fr";
const TABLO: &str = "https://github.com/y-l-g/tablo";
const TOPCOAT: &str = "https://github.com/tokio-rs/topcoat";
const TOASTY: &str = "https://github.com/tokio-rs/toasty";

/// Ledger's two faces: a grotesque for text, a mono for labels and code.
const SCHIBSTED_GROTESK: Font = fontsource_font!(SCHIBSTED_GROTESK, weight: [400, 600, 700, 800, 900], style: Normal, host: Asset);
const MARTIAN_MONO: Font =
    fontsource_font!(MARTIAN_MONO, weight: [400, 500, 600], style: Normal, host: Asset);

/// The parts of the template, a row each on the home page: its name, what it does, and the
/// folder or file that holds it.
const PARTS: [(&str, &str, &str); 8] = [
    ("site-part-teams", "site-part-teams-body", "src/teams/"),
    (
        "site-part-billing",
        "site-part-billing-body",
        "src/billing/",
    ),
    (
        "site-part-webhooks",
        "site-part-webhooks-body",
        "src/billing/webhook.rs",
    ),
    ("site-part-jobs", "site-part-jobs-body", "src/jobs/"),
    (
        "site-part-accounts",
        "site-part-accounts-body",
        "src/accounts/",
    ),
    (
        "site-part-backoffice",
        "site-part-backoffice-body",
        "src/backoffice/",
    ),
    (
        "site-part-support",
        "site-part-support-body",
        "src/support/",
    ),
    ("site-part-audit", "site-part-audit-body", "src/audit/"),
];

/// The domain a product replaces.
const PRODUCT_PART: &str = "src/projects/";

/// The masthead and footer every public page shares, around `content`.
fn frame<'a>(
    cx: &'a Cx,
    entered: &Cx,
    styled: bool,
    current: &'static str,
    content: BoxView<'a>,
) -> BoxView<'a> {
    // The view renders after this function returns, so it translates through a clone it owns.
    let t = {
        let cx = entered.clone();
        move |id: &'static str| tablo::t!(&cx, id)
    };
    let pricing_current = (current == "/pricing").then_some("page");
    view! {
        if styled {
            topcoat::font::link(font: SCHIBSTED_GROTESK)
            topcoat::font::link(font: MARTIAN_MONO)
        }
        <div class="ld">
            <div class="ld-sheet">
                <header class="ld-mast">
                    <a class="ld-brand" href="/">
                        mark()
                        (PRODUCT_NAME)
                    </a>
                    <nav class="ld-nav" aria-label=(t("site-nav"))>
                        <a href="/pricing" aria-current=(pricing_current)>(t("site-pricing"))</a>
                        <a class="ld-hide-sm" href=(SOURCE)>(t("site-source"))</a>
                        <a href="/app/login">(t("site-sign-in"))</a>
                        <a class="ld-nav-cta" href="/app/register">(t("site-sign-up"))</a>
                    </nav>
                </header>
                <main>(content)</main>
                <footer class="ld-foot">
                    <div class="ld-cells ld-cells-4">
                        <div class="ld-cell">
                            <h4>(PRODUCT_NAME)</h4>
                            <ul>
                                <li><a href="/pricing">(t("site-pricing"))</a></li>
                                <li><a href="/app/login">(t("site-sign-in"))</a></li>
                                <li><a href="/terms">(t("site-terms"))</a></li>
                                <li><a href="/privacy">(t("site-privacy"))</a></li>
                            </ul>
                        </div>
                        <div class="ld-cell">
                            <h4>(t("site-foot-template"))</h4>
                            <ul>
                                <li><a href=(SOURCE)>(t("site-source")) <small>"y-l-g/tablo-saas"</small></a></li>
                                <li><a href=(format!("{SOURCE}#run-it"))>(t("site-run-title"))</a></li>
                            </ul>
                        </div>
                        <div class="ld-cell">
                            <h4>(t("site-foot-built-on"))</h4>
                            <ul>
                                <li><a href=(TABLO_SITE)>(t("site-tablo-site"))</a></li>
                                <li><a href=(GUIDE)>(t("site-guide"))</a></li>
                                <li><a href=(TABLO)>"Tablo" <small>"y-l-g"</small></a></li>
                                <li><a href=(TOPCOAT)>"Topcoat" <small>"tokio-rs"</small></a></li>
                                <li><a href=(TOASTY)>"Toasty" <small>"tokio-rs"</small></a></li>
                            </ul>
                        </div>
                        <div class="ld-cell">
                            <h4>(t("site-foot-more"))</h4>
                            <ul>
                                <li><a href=(DEMO)>(t("site-demo"))</a></li>
                                <li><a href=(EXAMPLES)>(t("site-examples"))</a></li>
                            </ul>
                        </div>
                    </div>
                    <div class="ld-colophon">
                        <span>(t("site-built-with"))</span>
                        <span>"MIT"</span>
                    </div>
                </footer>
            </div>
        </div>
    }
    .boxed()
}

/// A plan as the site shows it.
struct PlanRow {
    name: String,
    price: String,
    yearly: Option<String>,
    seats: String,
    projects: String,
    audit: bool,
    support: bool,
}

/// Every plan, read from the catalog, so the site cannot disagree with what is sold.
fn plans(entered: &Cx) -> Vec<PlanRow> {
    let limit = |limit: Option<u32>| {
        limit.map_or_else(
            || tablo::t!(entered, "site-unlimited"),
            |limit| limit.to_string(),
        )
    };
    Plan::ALL
        .into_iter()
        .map(|plan| PlanRow {
            name: plan_name(entered, plan.id),
            price: if plan.monthly_amount == 0 {
                tablo::t!(entered, "site-price-free")
            } else {
                tablo::t!(
                    entered,
                    "billing-per-seat",
                    price = money(entered, plan.monthly_amount, CURRENCY)
                )
            },
            yearly: (plan.yearly_amount > 0).then(|| {
                tablo::t!(
                    entered,
                    "billing-yearly",
                    price = money(entered, plan.yearly_amount, CURRENCY)
                )
            }),
            seats: limit(plan.seat_limit),
            projects: limit(plan.project_limit),
            audit: plan.features.contains(&Feature::AuditLog),
            support: plan.features.contains(&Feature::PrioritySupport),
        })
        .collect()
}

/// `GET /`.
#[page("/")]
async fn home(cx: &Cx) -> Result<impl View> {
    let SitePanel { panel, styled } = context::<SitePanel>(cx);
    let entered = panel.enter(cx);
    let title = PRODUCT_NAME.to_string();
    let sections = [
        hero(cx, &entered),
        parts(cx, &entered),
        plans_row(cx, &entered),
        run(cx, &entered),
    ];
    let content = view! {
        for section in sections {
            (section)
        }
    }
    .boxed();
    panel
        .document(cx, title, frame(cx, &entered, *styled, "/", content))
        .await
}

/// The opening row: what the template is, how to get it, and a way in.
fn hero<'a>(cx: &'a Cx, entered: &Cx) -> BoxView<'a> {
    let t = {
        let cx = entered.clone();
        move |id: &'static str| tablo::t!(&cx, id)
    };
    view! {
    <section class="ld-row" aria-labelledby="site-title">
        <div class="ld-num"><b>"01"</b></div>
        <div class="ld-body">
            <div class="ld-hero">
                <p class="ld-label">(t("site-kicker"))</p>
                <h1 class="ld-h1" id="site-title">
                    (t("site-title-1"))
                    <br>
                    <span class="ld-red">(t("site-title-2"))</span>
                </h1>
            </div>
            <div class="ld-hero-sub">
                <div>
                    <p class="ld-lede">(t("site-lead"))</p>
                    <p class="ld-text">(t("site-live-note"))</p>
                </div>
                <div>
                    <div class="ld-cmd">
                        <code>"git clone https://github.com/y-l-g/tablo-saas"</code>
                    </div>
                    <div class="ld-actions">
                        <a class="ld-btn ld-btn-primary" href="/app/register">(t("site-start")) arrow()</a>
                        <a class="ld-btn" href=(SOURCE)>(t("site-read-source"))</a>
                    </div>
                    <p class="ld-meta">
                        <span>"MIT"</span>
                        <span>"PostgreSQL"</span>
                        <span>"Stripe"</span>
                        <span>(t("site-meta-locales"))</span>
                    </p>
                </div>
            </div>
        </div>
    </section>
    }
    .boxed()
}

/// What the template gives, a row each with the code that holds it.
fn parts<'a>(cx: &'a Cx, entered: &Cx) -> BoxView<'a> {
    let parts =
        PARTS.map(|(name, body, path)| (tablo::t!(entered, name), tablo::t!(entered, body), path));
    let t = {
        let cx = entered.clone();
        move |id: &'static str| tablo::t!(&cx, id)
    };
    view! {
    <section class="ld-row" aria-labelledby="site-parts">
        <div class="ld-num">"02"</div>
        <div class="ld-body">
            <div class="ld-head">
                <span id="site-parts">(t("site-parts-title"))</span>
                <span>(t("site-parts-note"))</span>
            </div>
            <table class="ld-table ld-table-stack">
                <thead>
                    <tr>
                        <th scope="col">(t("site-col-part"))</th>
                        <th scope="col">(t("site-col-does"))</th>
                        <th scope="col">(t("site-col-code"))</th>
                    </tr>
                </thead>
                <tbody>
                    for (name, body, path) in parts {
                        <tr>
                            <td>(name)</td>
                            <td data-th=(t("site-col-does"))>(body)</td>
                            <td class="ld-path" data-th=(t("site-col-code"))>
                                <a href=(format!("{TREE}{path}"))><code>(path)</code></a>
                            </td>
                        </tr>
                    }
                    <tr class="ld-marked">
                        <td>(t("site-part-product")) " " <span class="ld-tag">(t("site-replace"))</span></td>
                        <td data-th=(t("site-col-does"))>(t("site-part-product-body"))</td>
                        <td class="ld-path" data-th=(t("site-col-code"))>
                            <a href=(format!("{TREE}{PRODUCT_PART}"))><code>(PRODUCT_PART)</code></a>
                        </td>
                    </tr>
                </tbody>
            </table>
        </div>
    </section>
    }
    .boxed()
}

/// The plans, read from the catalog, with a link to compare them.
fn plans_row<'a>(cx: &'a Cx, entered: &Cx) -> BoxView<'a> {
    let plans = plans(entered);
    let t = {
        let cx = entered.clone();
        move |id: &'static str| tablo::t!(&cx, id)
    };
    view! {
    <section class="ld-row" aria-labelledby="site-plans">
        <div class="ld-num">"03"</div>
        <div class="ld-body">
            <div class="ld-head">
                <span id="site-plans">(t("site-pricing"))</span>
                <span>(t("site-plans-note"))</span>
            </div>
            <div class="ld-cells ld-cells-3">
                for plan in plans {
                    <div class="ld-cell">
                        <h2 class="ld-h3">(plan.name)</h2>
                        <p class="ld-text">(plan.price)</p>
                        <p class="ld-meta">
                            <span>(t("site-col-seats")) " " (plan.seats)</span>
                            <span>(t("site-col-projects")) " " (plan.projects)</span>
                        </p>
                    </div>
                }
            </div>
            <p class="ld-pad-sm ld-ruled"><a class="ld-link" href="/pricing">(t("site-compare"))</a></p>
        </div>
    </section>
    }
    .boxed()
}

/// The commands that start the template on a laptop.
fn run<'a>(cx: &'a Cx, entered: &Cx) -> BoxView<'a> {
    let t = {
        let cx = entered.clone();
        move |id: &'static str| tablo::t!(&cx, id)
    };
    view! {
    <section class="ld-row" aria-labelledby="site-run">
        <div class="ld-num">"04"</div>
        <div class="ld-body">
            <div class="ld-head">
                <span id="site-run">(t("site-run-title"))</span>
                <span>(t("site-run-note"))</span>
            </div>
            <div class="ld-cells ld-cells-2">
                <div class="ld-cell">
                    <p class="ld-text">(t("site-run-body"))</p>
                    <p class="ld-text">(t("site-run-billing"))</p>
                </div>
                <div class="ld-cell">
                    <pre class="ld-code">
                        <span><span class="c">"# "(t("site-run-env"))</span></span>
                        <span><span class="k">"$ "</span>"cp .env.example .env"</span>
                        <span><span class="c">"# "(t("site-run-services"))</span></span>
                        <span><span class="k">"$ "</span>"docker compose up -d"</span>
                        <span><span class="c">"# "(t("site-run-app"))</span></span>
                        <span><span class="k">"$ "</span>"topcoat dev"</span>
                    </pre>
                </div>
            </div>
        </div>
    </section>
    }
    .boxed()
}

/// `GET /pricing`, read from the plan catalog, so the page cannot disagree with what is sold.
#[page("/pricing")]
async fn pricing(cx: &Cx) -> Result<impl View> {
    let SitePanel { panel, styled } = context::<SitePanel>(cx);
    let entered = panel.enter(cx);
    let title = tablo::t!(&entered, "site-pricing");
    let label = title.clone();
    let plans = plans(&entered);
    let grant = |on: bool| {
        if on {
            ("yes", tablo::t!(&entered, "site-included"))
        } else {
            ("none", tablo::t!(&entered, "site-not-included"))
        }
    };
    let rows: Vec<_> = plans
        .into_iter()
        .map(|plan| (grant(plan.audit), grant(plan.support), plan))
        .collect();
    let t = {
        let cx = entered.clone();
        move |id: &'static str| tablo::t!(&cx, id)
    };
    let content = view! {
        <section class="ld-row" aria-labelledby="pricing-title">
            <div class="ld-num"><b>"01"</b></div>
            <div class="ld-body">
                <div class="ld-hero">
                    <p class="ld-label">(label)</p>
                    <h1 class="ld-h1" id="pricing-title">
                        (t("site-pricing-title-1"))
                        <br>
                        <span class="ld-red">(t("site-pricing-title-2"))</span>
                    </h1>
                </div>
                <table class="ld-table ld-table-stack">
                    <thead>
                        <tr>
                            <th scope="col">(t("site-col-plan"))</th>
                            <th scope="col">(t("site-col-price"))</th>
                            <th scope="col">(t("site-col-seats"))</th>
                            <th scope="col">(t("site-col-projects"))</th>
                            <th scope="col" class="ld-grants">(t("site-col-audit"))</th>
                            <th scope="col" class="ld-grants">(t("site-col-support"))</th>
                        </tr>
                    </thead>
                    <tbody>
                        for ((audit, audit_label), (support, support_label), plan) in rows {
                            <tr>
                                <td>(plan.name)</td>
                                <td data-th=(t("site-col-price"))>
                                    <b>(plan.price)</b>
                                    if let Some(yearly) = plan.yearly {
                                        <br>
                                        <small class="ld-label">(yearly)</small>
                                    }
                                </td>
                                <td data-th=(t("site-col-seats"))>(plan.seats)</td>
                                <td data-th=(t("site-col-projects"))>(plan.projects)</td>
                                <td class="ld-grants" data-th=(t("site-col-audit"))>
                                    <span class="ld-grant" data-level=(audit)>(audit_label)</span>
                                </td>
                                <td class="ld-grants" data-th=(t("site-col-support"))>
                                    <span class="ld-grant" data-level=(support)>(support_label)</span>
                                </td>
                            </tr>
                        }
                    </tbody>
                </table>
                <div class="ld-hero-sub">
                    <div><p class="ld-text">(t("site-pricing-body"))</p></div>
                    <div>
                        <div class="ld-actions">
                            <a class="ld-btn ld-btn-primary" href="/app/register">(t("site-start")) arrow()</a>
                        </div>
                    </div>
                </div>
            </div>
        </section>
    }
    .boxed();
    panel
        .document(cx, title, frame(cx, &entered, *styled, "/pricing", content))
        .await
}

/// A page of prose: a title and a sample the owner must replace before launch.
async fn legal(
    cx: &Cx,
    path: &'static str,
    title_id: &'static str,
    body_id: &'static str,
) -> Result<impl View> {
    let SitePanel { panel, styled } = context::<SitePanel>(cx);
    let entered = panel.enter(cx);
    let title = tablo::t!(&entered, title_id);
    let heading = title.clone();
    let body = tablo::t!(&entered, body_id);
    let content = view! {
        <section class="ld-row" aria-labelledby="legal-title">
            <div class="ld-num"><b>"01"</b></div>
            <div class="ld-body">
                <div class="ld-hero">
                    <h1 class="ld-h2" id="legal-title">(heading)</h1>
                    <p class="ld-lede">(body)</p>
                </div>
            </div>
        </section>
    }
    .boxed();
    panel
        .document(cx, title, frame(cx, &entered, *styled, path, content))
        .await
}

/// `GET /terms`.
#[page("/terms")]
async fn terms(cx: &Cx) -> Result<impl View> {
    legal(cx, "/terms", "site-terms", "site-terms-body").await
}

/// `GET /privacy`.
#[page("/privacy")]
async fn privacy(cx: &Cx) -> Result<impl View> {
    legal(cx, "/privacy", "site-privacy", "site-privacy-body").await
}

/// Tablo's mark: a three-by-three grid whose header row is ink and whose middle column is red,
/// which draws a T.
#[component]
async fn mark(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <svg viewBox="0 0 32 32" aria-hidden="true">
            <rect x="1" y="1" width="30" height="30" fill="var(--ld-sheet)" stroke="var(--ld-ink)" stroke-width="2"></rect>
            <rect x="1" y="1" width="30" height="10" fill="var(--ld-ink)"></rect>
            <rect x="11" y="11" width="10" height="20" fill="var(--ld-red)"></rect>
            <path d="M11 11v20M21 11v20M1 21h10M21 21h10" fill="none" stroke="var(--ld-ink)" stroke-width="1.5"></path>
        </svg>
    })
}

/// The arrow a call to action ends on.
#[component]
async fn arrow(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M2 8h11M9 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.6"></path>
        </svg>
    })
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

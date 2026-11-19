//! The public website, outside every panel. It borrows the `app` panel's document and catalogs, so
//! it speaks the visitor's language and follows the theme picked in the app, and its masthead
//! switches both.
//!
//! It wears Ledger, the identity Tablo's site and demo share (`ledger.css`), and presents the
//! template as it ships. A product built on the template replaces the copy under `## Website` in
//! `locales/`, and `ledger.css` with its own brand if it wants one.

use serde::Deserialize;
use tablo::PanelHandle;
use topcoat::{
    Result,
    context::{Cx, base_url, context},
    cookie::{Cookie, Cookies, SameSite, cookies},
    font::{Font, fontsource::fontsource_font},
    icon::icon,
    router::{
        HeaderMap, HeaderValue,
        content::Form,
        error::{SeeOther, forbidden, see_other},
        header::{CONTENT_TYPE, SET_COOKIE},
        page, route,
    },
    view::{BoxView, View, ViewExt, component, view},
};

use crate::billing::{CURRENCY, Feature, Plan, money, plan_name};
use crate::panels::{LOGO_URL, PRODUCT_NAME};

/// The mounted panels: the one whose document and catalogs the site borrows, and the one it only
/// links to.
#[derive(Clone)]
pub struct SitePanel {
    /// The `app` panel, whose document wraps every public page.
    pub panel: PanelHandle,
    /// The `admin` panel, the back office, which the site links to.
    pub admin: PanelHandle,
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

/// The panels' locales, each named in its own language.
const LANGUAGES: [(&str, &str); 2] = [("en", "English"), ("fr", "Français")];

/// The cookies Tablo reads a visitor's language from, and its `theme.js` their theme.
///
/// Tablo writes the language one under each panel's prefix, which the site never receives, so the
/// site keeps its own copy at `/` and writes the panels' along with it.
const LOCALE_COOKIE: &str = "tablo_locale";
const THEME_COOKIE: &str = "tablo_theme";

/// Where each panel is mounted, read from the router, for the links that lead into them.
struct Panels {
    app: String,
    admin: String,
}

impl Panels {
    fn of(site: &SitePanel) -> Self {
        Self {
            app: site.panel.url().unwrap_or_else(|| "/app".to_string()),
            admin: site.admin.url().unwrap_or_else(|| "/admin".to_string()),
        }
    }
}

/// The masthead and footer every public page shares, around `content`.
fn frame<'a>(
    cx: &'a Cx,
    entered: &Cx,
    site: &SitePanel,
    current: &'static str,
    content: BoxView<'a>,
) -> BoxView<'a> {
    let styled = site.styled;
    let Panels { app, admin } = Panels::of(site);
    let locale = topcoat::l10n::try_locale(entered).map(ToString::to_string);
    // The document is the app panel's, which states no language, so the page states its own.
    let lang = locale.unwrap_or_else(|| "en".to_string());
    // The view renders after this function returns, so it translates through a clone it owns.
    let t = {
        let cx = entered.clone();
        move |id: &'static str| tablo::t!(&cx, id)
    };
    let pricing_current = (current == "/pricing").then_some("page");
    let app_login = format!("{app}/login");
    let admin_login = format!("{admin}/login");
    let sign_up = format!("{app}/register");
    let switches = switches(cx, entered);
    view! {
        if styled {
            topcoat::font::link(font: SCHIBSTED_GROTESK)
            topcoat::font::link(font: MARTIAN_MONO)
        }
        <div lang=(lang) class="ld">
            <div class="ld-sheet">
                <header class="ld-mast">
                    <a class="ld-brand" href="/">
                        <img src=(LOGO_URL) width="32" height="32" alt="">
                        (PRODUCT_NAME)
                    </a>
                    <nav class="ld-nav" aria-label=(t("site-nav"))>
                        <a class="ld-hide-sm" href="/pricing" aria-current=(pricing_current)>(t("site-pricing"))</a>
                        <a class="ld-hide-sm" href=(SOURCE)>(t("site-source"))</a>
                        (switches)
                        <a class="ld-hide-sm" href=(app_login.clone())>(t("site-sign-in"))</a>
                        <a class="ld-nav-cta ld-hide-sm" href=(sign_up)>(t("site-sign-up"))</a>
                    </nav>
                </header>
                <main>(content)</main>
                <footer class="ld-foot">
                    <div class="ld-cells ld-cells-4">
                        <div class="ld-cell">
                            <h4>(PRODUCT_NAME)</h4>
                            <ul>
                                <li><a href="/pricing">(t("site-pricing"))</a></li>
                                <li><a href=(app_login)>(t("site-sign-in"))</a></li>
                                <li><a href=(admin_login)>(t("site-panel-admin-title"))</a></li>
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

/// The masthead's switches: the language, for the site and both panels, and the theme.
fn switches<'a>(cx: &'a Cx, entered: &Cx) -> BoxView<'a> {
    let locale = topcoat::l10n::try_locale(entered).map(ToString::to_string);
    // The language switch posts a form, which checks this token against its cookie.
    let csrf = tablo::csrf::ensure_token(cx);
    let languages = LANGUAGES.map(|(tag, name)| (tag, name, locale.as_deref() == Some(tag)));
    // `theme.js` checks the theme picked on click; the page checks the stored one. The panels
    // default to light.
    let stored = cookies(cx).get(THEME_COOKIE);
    let stored = stored.as_ref().map_or("light", |cookie| cookie.value());
    let themes = [
        ("light", tablo::ui::icons::SUN, "tablo-shell-theme-light"),
        ("dark", tablo::ui::icons::MOON, "tablo-shell-theme-dark"),
        (
            "system",
            tablo::ui::icons::MONITOR,
            "tablo-shell-theme-system",
        ),
    ]
    .map(|(name, data, label)| (name, data, tablo::t!(entered, label), name == stored));
    let t = {
        let cx = entered.clone();
        move |id: &'static str| tablo::t!(&cx, id)
    };
    view! {
        <div class="ld-seg" role="group" aria-label=(t("tablo-shell-language"))>
            for (tag, name, current) in languages {
                <form method="post" action="/locale">
                    (tablo::csrf::field(cx, &csrf))
                    <input type="hidden" name="locale" value=(tag)>
                    <button type="submit" title=(name) lang=(tag) aria-current=(current.then_some("true"))>
                        (tag)
                    </button>
                </form>
            }
        </div>
        <div class="ld-seg" role="radiogroup" aria-label=(t("tablo-shell-theme"))>
            for (name, data, label, checked) in themes {
                <button
                    type="button"
                    role="radio"
                    data-theme-set=(name)
                    aria-checked=(if checked { "true" } else { "false" })
                    aria-label=(label.clone())
                    title=(label)
                >
                    icon(data: data)
                </button>
            }
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
    let site = context::<SitePanel>(cx);
    let entered = site.panel.enter(cx);
    let title = PRODUCT_NAME.to_string();
    let sections = [
        hero(cx, &entered),
        panels(cx, &entered, Panels::of(site)),
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
    site.panel
        .document(cx, title, frame(cx, &entered, site, "/", content))
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

/// The two panels the template mounts, and the way into each.
fn panels<'a>(cx: &'a Cx, entered: &Cx, panels: Panels) -> BoxView<'a> {
    let Panels { app, admin } = panels;
    let app_login = format!("{app}/login");
    let sign_up = format!("{app}/register");
    let admin_login = format!("{admin}/login");
    let t = {
        let cx = entered.clone();
        move |id: &'static str| tablo::t!(&cx, id)
    };
    view! {
    <section class="ld-row" aria-labelledby="site-panels">
        <div class="ld-num">"02"</div>
        <div class="ld-body">
            <div class="ld-head">
                <span id="site-panels">(t("site-panels-title"))</span>
                <span>(t("site-panels-note"))</span>
            </div>
            <div class="ld-cells ld-cells-2">
                <div class="ld-cell">
                    <p class="ld-label">(app)</p>
                    <h2 class="ld-h3">(t("site-panel-app-title"))</h2>
                    <p class="ld-text">(t("site-panel-app-body"))</p>
                    <div class="ld-actions">
                        <a class="ld-btn" href=(app_login)>(t("site-sign-in")) arrow()</a>
                    </div>
                    <p><a class="ld-link" href=(sign_up)>(t("site-panel-app-sign-up"))</a></p>
                </div>
                <div class="ld-cell">
                    <p class="ld-label">(admin)</p>
                    <h2 class="ld-h3">(t("site-panel-admin-title"))</h2>
                    <p class="ld-text">(t("site-panel-admin-body"))</p>
                    <div class="ld-actions">
                        <a class="ld-btn" href=(admin_login)>(t("site-sign-in")) arrow()</a>
                    </div>
                    <div class="ld-cmd">
                        <code>"cargo run -- staff grant you@example.com --role administrator"</code>
                    </div>
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
        <div class="ld-num">"03"</div>
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
        <div class="ld-num">"04"</div>
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
        <div class="ld-num">"05"</div>
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
    let site = context::<SitePanel>(cx);
    let entered = site.panel.enter(cx);
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
    site.panel
        .document(cx, title, frame(cx, &entered, site, "/pricing", content))
        .await
}

/// A page of prose: a title and a sample the owner must replace before launch.
async fn legal(
    cx: &Cx,
    path: &'static str,
    title_id: &'static str,
    body_id: &'static str,
) -> Result<impl View> {
    let site = context::<SitePanel>(cx);
    let entered = site.panel.enter(cx);
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
    site.panel
        .document(cx, title, frame(cx, &entered, site, path, content))
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

/// The arrow a call to action ends on.
#[component]
async fn arrow(cx: &Cx) -> Result<impl View> {
    Ok(view! {
        <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M2 8h11M9 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.6"></path>
        </svg>
    })
}

/// The language picked from the masthead's switch.
#[derive(Deserialize)]
struct LocaleForm {
    csrf_token: String,
    locale: String,
}

/// `POST /locale`: read the site and both panels in another language, then land back on `/`.
#[route(POST "/locale")]
async fn pick_locale(cx: &Cx, Form(form): Form<LocaleForm>) -> Result<(HeaderMap, SeeOther)> {
    tablo::csrf::verify(cx, Some(&form.csrf_token))?;
    if !LANGUAGES.iter().any(|(tag, _)| *tag == form.locale) {
        return Err(forbidden().into());
    }
    let site = context::<SitePanel>(cx);
    // The panels' own copies too, which outrank this one under their prefixes. The cookie jar
    // keeps one cookie per name, so they go out as headers of their own.
    let paths = ["/".to_string()]
        .into_iter()
        .chain(site.panel.url())
        .chain(site.admin.url());
    let mut headers = HeaderMap::new();
    for path in paths {
        let cookie = Cookie::build((LOCALE_COOKIE, form.locale.clone()))
            .path(path)
            .http_only(true)
            .secure(true)
            .same_site(SameSite::Lax)
            .max_age(topcoat::cookie::time::Duration::days(365))
            .build();
        headers.append(SET_COOKIE, HeaderValue::try_from(cookie.to_string())?);
    }
    Ok((headers, see_other("/")))
}

fn typed(kind: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(kind));
    headers
}

/// `GET /robots.txt`: the site is public and the panels are not.
#[route(GET "/robots.txt")]
async fn robots(cx: &Cx) -> Result<(HeaderMap, String)> {
    let map = base_url(cx).join("/sitemap.xml");
    Ok((
        typed("text/plain; charset=utf-8"),
        format!("User-agent: *\nDisallow: /app\nDisallow: /admin\nSitemap: {map}\n"),
    ))
}

/// `GET /sitemap.xml`.
#[route(GET "/sitemap.xml")]
async fn sitemap(cx: &Cx) -> Result<(HeaderMap, String)> {
    let base = base_url(cx);
    let urls = PUBLIC_PATHS.map(|path| format!("<url><loc>{}</loc></url>", base.join(path)));
    let urls = urls.concat();
    Ok((
        typed("application/xml"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">{urls}</urlset>"#
        ),
    ))
}

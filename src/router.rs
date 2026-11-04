//! The app's router: the routes it discovers, the state they read and the panels it mounts.

use tablo::{PanelHandle, RouterBuilderPanelExt};
use toasty::Db;
use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    font::{Font, fontsource::fontsource_font},
    mail::{MailConfig, RouterBuilderMailExt, Transport},
    router::{Router, RouterBuilderDiscoverExt, TrustedProxies},
    tailwind,
};

use crate::panels::app_panel;
use crate::platform::{config::Config, http::Https};

/// The theme's sans font.
const INTER: Font = fontsource_font!(INTER, host: Asset);

/// What the router is built from: the parts a test swaps.
pub struct App<'a, T> {
    pub db: Db,
    pub config: &'a Config,
    pub mail: T,
    /// The bundle `topcoat asset bundle` wrote. Without one, pages render unstyled and work.
    pub assets: Option<AssetBundle>,
}

/// The router, and the handle a background job reads the panel through.
pub struct Built {
    pub router: Router,
    pub app_panel: PanelHandle,
}

/// Builds the router the server runs and the tests drive.
pub fn router<T: Transport + 'static>(app: App<'_, T>) -> topcoat::Result<Built> {
    let App {
        db,
        config,
        mail,
        assets,
    } = app;
    let mut builder = Router::builder()
        .discover()
        .with(db)
        .with(config.cookie_key.clone())
        .with(Https(config.base_url.starts_with("https://")))
        .with(config.settings())
        .mail(MailConfig::builder().transport(mail).build());
    if !config.trusted_proxies.is_empty() {
        builder =
            builder.trusted_proxies(TrustedProxies::new().networks(config.trusted_proxies.clone()));
    }
    let styled = assets.is_some();
    if let Some(assets) = assets {
        builder = builder.assets(assets);
    }
    let shell = |panel: tablo::Panel| {
        if styled {
            panel.shell_assets(tailwind::stylesheet!(), INTER)
        } else {
            panel
        }
    };
    let builder = builder.panel(shell(app_panel(config)))?;
    let app_panel = builder.panel_handle("app").expect("mounted above");
    Ok(Built {
        router: builder.build(),
        app_panel,
    })
}

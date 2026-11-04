//! The panels: `app`, where customers work.

mod app;
mod brand;

pub use app::app_panel;
pub use brand::PRODUCT_NAME;

use tablo::Panel;
use topcoat::l10n::locale;

/// What every panel shares: the brand and the languages, the browser choosing between them.
fn shared(panel: Panel) -> Panel {
    panel
        .brand(brand::brand())
        .locales([locale!("en"), locale!("fr")])
        .translations(locale!("en"), include_str!("../../locales/en.ftl"))
        .translations(locale!("fr"), include_str!("../../locales/fr.ftl"))
}

//! The strip above every page of the app panel when the team's payment needs its owner.

use tablo::Banner;
use topcoat::{
    Result,
    context::Cx,
    router::request,
    view::{View, view},
};

use super::{Standing, page::URL, standing_notice};
use crate::teams::{current_entitlements, may_manage_members};

/// Whether `standing` must reach the team wherever it works, not only on the billing page: a
/// failed payment counts down to the Free plan, and a lapsed team has lost its plan.
pub fn is_urgent(standing: Standing) -> bool {
    matches!(standing, Standing::Grace(_) | Standing::Lapsed)
}

/// A failed or lapsed payment, linking to the billing page, for those who may open it. The page
/// says it itself, so it shows none there.
pub struct StandingBanner;

impl Banner for StandingBanner {
    async fn render(cx: &Cx) -> Result<Option<impl View>> {
        let standing = current_entitlements(cx).standing;
        let on_billing = request::uri(cx).path().trim_end_matches('/') == URL;
        if !is_urgent(standing) || !may_manage_members(cx) || on_billing {
            return Ok(None);
        }
        Ok(standing_notice(cx, standing).map(|notice| {
            view! {
                <div data-billing-banner="" role="alert" class="border-b border-border bg-muted px-4 py-2 text-sm">
                    <a href=(URL) class="underline">(notice)</a>
                </div>
            }
        }))
    }
}

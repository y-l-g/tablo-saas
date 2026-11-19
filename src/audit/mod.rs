//! Who did what, kept by `tablo::audit`: each panel writes an [`AuditEntry`] per record a write
//! names, in the write's own transaction, with the fields it changed. A team reads its own trail on
//! a plan with [`Feature::AuditLog`]; the trail is written on every plan, so upgrading shows
//! history. Staff read the back office's.
//!
//! An entry outlives the team or the account it names unless [`tablo::audit::forget_tenant`] or
//! [`tablo::audit::forget_actor`] removes it, as erasing either does. The actions that erase both
//! are page header actions, which run no commit hook, so the log keeps no entry of its own for
//! them.

use tablo::{ResourceDef, audit::AuditLog, auth, when};
use topcoat::{context::Cx, icon::iconify::iconify_icon};

use crate::accounts::{SignedIn, StaffRole};
use crate::billing::Feature;
use crate::teams::{current_entitlements, may_manage_members};

/// The team's trail, for those who manage it on a plan that includes it. The app panel places it
/// in the team's settings, beside members and billing.
pub fn team_trail(def: ResourceDef<AuditLog>) -> ResourceDef<AuditLog> {
    def.icon(iconify_icon!("lucide:rotate-ccw-clock"))
        .policy(when(reads_the_trail))
        .without_global_search()
}

fn reads_the_trail(cx: &Cx) -> bool {
    may_manage_members(cx) && current_entitlements(cx).has(Feature::AuditLog)
}

/// The back office's trail, for administrators.
pub fn staff_trail(def: ResourceDef<AuditLog>) -> ResourceDef<AuditLog> {
    def.icon(iconify_icon!("lucide:rotate-ccw-clock"))
        .policy(when(|cx| {
            auth::user::<SignedIn>(cx)
                .is_some_and(|signed| signed.user.staff_role == Some(StaffRole::Administrator))
        }))
        .without_global_search()
}

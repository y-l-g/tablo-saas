//! Who did what, kept by Tablo's audit log: each panel writes an [`AuditEntry`] per record a write
//! names, in the write's own transaction, with the fields it changed. A team reads its own trail on
//! a plan with [`Feature::AuditLog`]; the trail is written on every plan, so upgrading shows
//! history. Staff read the back office's.
//!
//! What this module adds is erasure: an entry outlives the team or the account it names unless
//! [`forget_team`] or [`forget_actor`] removes it. The actions that erase both are page header
//! actions, which run no commit hook, so the log keeps no entry of its own for them.

use tablo::{AuditEntry, AuditLog, ReadOnly, ResourceDef, TenantId, auth, when};
use topcoat::{Result, context::Cx, icon::iconify::iconify_icon};

use crate::accounts::{SignedIn, StaffRole};
use crate::billing::Feature;
use crate::teams::{current_entitlements, may_manage_members};

/// The team's trail, for those who manage it on a plan that includes it. The app panel places it
/// in the team's settings, beside members and billing.
pub fn team_trail(def: ResourceDef<AuditLog>) -> ResourceDef<AuditLog> {
    def.icon(iconify_icon!("lucide:rotate-ccw-clock"))
        .policy(ReadOnly.and(when(reads_the_trail)))
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

/// Deletes every entry of `team`, for a team being erased.
///
/// # Errors
///
/// When the delete fails.
pub async fn forget_team(ex: &mut dyn toasty::Executor, team: TenantId) -> Result<()> {
    AuditEntry::filter(AuditEntry::fields().tenant().eq(Some(team)))
        .delete()
        .exec(ex)
        .await?;
    Ok(())
}

/// Keeps what `user` did on other teams, but no longer who did it.
///
/// # Errors
///
/// When the update fails.
pub async fn forget_actor(cx: &Cx, ex: &mut dyn toasty::Executor, user: uuid::Uuid) -> Result<()> {
    AuditEntry::filter(AuditEntry::fields().actor_id().eq(Some(user.to_string())))
        .update()
        .actor_id(None::<String>)
        .actor(tablo::t!(cx, "audit-deleted-actor"))
        .exec(ex)
        .await?;
    Ok(())
}

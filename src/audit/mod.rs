//! Who did what. Each panel's commit hook writes its row in the write's own transaction, so a
//! change cannot commit unrecorded. A team reads its own trail on a plan with
//! [`Feature::AuditLog`]; the trail is written on every plan, so upgrading shows history.

use jiff::Timestamp;
use tablo::{
    Change, ChangedRecord, CommitHook, PanelChange, TenantId, auth, prelude::*, tenant_id,
};
use topcoat::{Result, context::Cx, icon::iconify::iconify_icon};

use crate::accounts::{SignedIn, StaffRole};
use crate::billing::Feature;
use crate::teams::{current_entitlements, may_manage_members};

/// A write a team member made.
#[derive(Debug, Clone, toasty::Model, tablo::ModelColumns)]
#[table = "audit_entries"]
pub struct AuditEntry {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[index]
    pub team_id: TenantId,
    pub actor: String,
    pub resource: String,
    pub change: String,
    pub record: String,
    pub at: Timestamp,
}

/// A write a staff member made in the back office.
#[derive(Debug, Clone, toasty::Model, tablo::ModelColumns)]
#[table = "staff_audit_entries"]
pub struct StaffAuditEntry {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    pub actor: String,
    pub resource: String,
    pub change: String,
    pub record: String,
    pub at: Timestamp,
}

/// What a write was, as the trail spells it. It matches every kind, so a kind Tablo adds stops the
/// build instead of going unrecorded.
fn described(change: &PanelChange) -> String {
    match change.change() {
        Change::Create(_) => "create".to_string(),
        Change::Update { .. } => "update".to_string(),
        Change::Delete(_) => "delete".to_string(),
        Change::Restore(_) => "restore".to_string(),
        Change::ForceDelete(_) => "force delete".to_string(),
        Change::Action { name, .. } | Change::HeaderAction { name } => format!("action: {name}"),
        Change::Attach { .. } => "attach".to_string(),
        Change::Detach { .. } => "detach".to_string(),
    }
}

fn actor(cx: &Cx) -> String {
    auth::user::<SignedIn>(cx).map_or_else(
        || "anonymous".to_string(),
        |signed| signed.user.email.clone(),
    )
}

/// The first record a write names: a bulk write links the first of its rows.
fn record(change: &PanelChange) -> String {
    change
        .change()
        .records()
        .first()
        .and_then(ChangedRecord::url)
        .unwrap_or_default()
        .to_string()
}

/// Records the `app` panel's writes against the team acting.
pub struct TeamAudit;

impl CommitHook for TeamAudit {
    async fn before_commit(
        cx: &Cx,
        change: &PanelChange,
        ex: &mut dyn toasty::Executor,
    ) -> Result<()> {
        // A write with no team acting has no trail to land on.
        let Some(team) = tenant_id(cx) else {
            return Ok(());
        };
        toasty::create!(AuditEntry {
            team_id: team,
            actor: actor(cx),
            resource: change.label().localize(cx),
            change: described(change),
            record: record(change),
            at: Timestamp::now(),
        })
        .exec(ex)
        .await?;
        Ok(())
    }
}

/// Records the back office's writes.
pub struct StaffAudit;

impl CommitHook for StaffAudit {
    async fn before_commit(
        cx: &Cx,
        change: &PanelChange,
        ex: &mut dyn toasty::Executor,
    ) -> Result<()> {
        toasty::create!(StaffAuditEntry {
            actor: actor(cx),
            resource: change.label().localize(cx),
            change: described(change),
            record: record(change),
            at: Timestamp::now(),
        })
        .exec(ex)
        .await?;
        Ok(())
    }
}

/// The team's trail, for those who manage it on a plan that includes it.
pub struct AuditResource;

fn reads_the_trail(cx: &Cx) -> bool {
    may_manage_members(cx) && current_entitlements(cx).has(Feature::AuditLog)
}

impl Resource for AuditResource {
    type Model = AuditEntry;
    type Form = NoForm<AuditEntry>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("audit-log")
            .icon(iconify_icon!("lucide:rotate-ccw-clock"))
            .navigation_group(tablo::text!("navigation-team"))
            .tenancy(Tenancy::column(lens!(AuditEntry.team_id)))
            .policy(ReadOnly.and(when(reads_the_trail)))
            .without_page(ResourcePage::View)
            .without_global_search()
            .table(
                Table::new((
                    DateColumn::new(lens!(AuditEntry.at)).sortable(),
                    TextColumn::new(lens!(AuditEntry.actor)).searchable(),
                    TextColumn::new(lens!(AuditEntry.resource)),
                    TextColumn::new(lens!(AuditEntry.change)),
                ))
                .default_sort_desc(lens!(AuditEntry.at)),
            )
    }
}

/// The back office's trail, for administrators.
pub struct StaffAuditResource;

impl Resource for StaffAuditResource {
    type Model = StaffAuditEntry;
    type Form = NoForm<StaffAuditEntry>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("staff-audit-log")
            .icon(iconify_icon!("lucide:rotate-ccw-clock"))
            .tenancy(Tenancy::none())
            .policy(ReadOnly.and(when(|cx| {
                auth::user::<SignedIn>(cx)
                    .is_some_and(|signed| signed.user.staff_role == Some(StaffRole::Administrator))
            })))
            .without_page(ResourcePage::View)
            .without_global_search()
            .table(
                Table::new((
                    DateColumn::new(lens!(StaffAuditEntry.at)).sortable(),
                    TextColumn::new(lens!(StaffAuditEntry.actor)).searchable(),
                    TextColumn::new(lens!(StaffAuditEntry.resource)),
                    TextColumn::new(lens!(StaffAuditEntry.change)),
                ))
                .default_sort_desc(lens!(StaffAuditEntry.at)),
            )
    }
}

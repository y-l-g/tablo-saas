//! The right of access: one JSON file of what an account owns, the rows [`super::deletion`]
//! erases. Each list on a page exports as CSV through Tablo; only the app knows which rows across
//! them are one person's.
//!
//! The file names no other person beyond what the account already reads: a team it owns brings
//! its projects, invitations and billing, but not its members' accounts.

use jiff::Timestamp;
use serde::Serialize;
use tablo::{
    HeaderAction, Notification, Outcome, TenantId, WriteError,
    audit::{AuditChange, AuditEntry},
    auth,
    db::db,
    notification::StoredNotification,
};
use topcoat::{
    Result,
    context::Cx,
    router::{
        HeaderMap, HeaderValue,
        header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE},
        route,
    },
};

use super::{Invitation, Member, Team, TeamRole};
use crate::accounts::{Identity, SignedIn, StaffRole, User};
use crate::billing::{BillingAccount, BillingInterval, BillingStatus, PlanId};
use crate::projects::{Project, ProjectStatus};

/// Where the file downloads from.
pub const EXPORT_URL: &str = "/app/settings/profile/export";

/// Everything the signed-in account owns, as the download spells it.
#[derive(Serialize)]
pub struct AccountExport {
    pub exported_at: Timestamp,
    pub user: UserExport,
    /// The Google accounts that sign in as this one.
    pub identities: Vec<IdentityExport>,
    pub memberships: Vec<MembershipExport>,
    /// The teams the account owns, with what deleting it would erase or hand over.
    pub owned_teams: Vec<TeamExport>,
    /// Invitations addressed to the account, from any team.
    pub invitations: Vec<InvitationExport>,
    /// What the account did, as every team's audit trail recorded it.
    pub audit_entries: Vec<AuditExport>,
    pub notifications: Vec<NotificationExport>,
}

/// The account itself, without its password hash.
#[derive(Serialize)]
pub struct UserExport {
    pub id: uuid::Uuid,
    pub name: String,
    pub email: String,
    pub email_verified_at: Option<Timestamp>,
    pub staff_role: Option<StaffRole>,
    pub staff_since: Option<Timestamp>,
    pub locale: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// A provider's account linked to this one: which provider, its id for the account there, and
/// when it was linked.
#[derive(Serialize)]
pub struct IdentityExport {
    pub issuer: String,
    pub subject: String,
    pub linked_at: Timestamp,
}

impl From<Identity> for IdentityExport {
    fn from(identity: Identity) -> Self {
        Self {
            issuer: identity.issuer,
            subject: identity.subject,
            linked_at: identity.created_at,
        }
    }
}

/// One seat on one team.
#[derive(Serialize)]
pub struct MembershipExport {
    pub team_id: uuid::Uuid,
    pub team_name: String,
    pub role: TeamRole,
    pub joined_at: Timestamp,
}

/// A team the account owns.
#[derive(Serialize)]
pub struct TeamExport {
    pub id: uuid::Uuid,
    pub name: String,
    pub created_at: Timestamp,
    pub projects: Vec<ProjectExport>,
    /// Who the team has invited and not yet heard from.
    pub invitations: Vec<InvitationExport>,
    pub billing_account: Option<BillingExport>,
}

/// One of an owned team's projects.
#[derive(Serialize)]
pub struct ProjectExport {
    pub id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: ProjectStatus,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// An invitation, sent or received.
#[derive(Serialize)]
pub struct InvitationExport {
    pub team_id: uuid::Uuid,
    pub team_name: String,
    pub email: String,
    pub role: TeamRole,
    pub invited_by: String,
    pub expires_at: Timestamp,
    pub created_at: Timestamp,
}

/// An owned team's mirror of its Stripe subscription.
#[derive(Serialize)]
pub struct BillingExport {
    pub customer_id: String,
    pub plan: PlanId,
    pub status: BillingStatus,
    pub subscription_id: Option<String>,
    pub billing_interval: Option<BillingInterval>,
    pub seats: Option<i64>,
    pub current_period_end: Option<Timestamp>,
    pub cancel_at_period_end: bool,
    pub trial_ends_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

/// One thing the account did.
#[derive(Serialize)]
pub struct AuditExport {
    pub panel: String,
    pub team_id: Option<uuid::Uuid>,
    pub event: String,
    pub resource: Option<String>,
    pub record: Option<String>,
    pub changes: Vec<AuditChange>,
    pub created_at: Timestamp,
}

/// A message the bell kept for the account.
#[derive(Serialize)]
pub struct NotificationExport {
    pub panel: String,
    pub team_id: Option<uuid::Uuid>,
    pub notification: Notification,
    pub read_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

impl From<User> for UserExport {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            name: user.name,
            email: user.email,
            email_verified_at: user.email_verified_at,
            staff_role: user.staff_role,
            staff_since: user.staff_since,
            locale: user.locale,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

impl From<Project> for ProjectExport {
    fn from(project: Project) -> Self {
        Self {
            id: project.id,
            name: project.name,
            description: project.description,
            status: project.status,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

impl From<Invitation> for InvitationExport {
    fn from(invitation: Invitation) -> Self {
        Self {
            team_id: invitation.team_id.get(),
            team_name: invitation.team_name,
            email: invitation.email,
            role: invitation.role,
            invited_by: invitation.invited_by,
            expires_at: invitation.expires_at,
            created_at: invitation.created_at,
        }
    }
}

impl From<BillingAccount> for BillingExport {
    fn from(account: BillingAccount) -> Self {
        Self {
            customer_id: account.customer_id,
            plan: account.plan,
            status: account.status,
            subscription_id: account.subscription_id,
            billing_interval: account.billing_interval,
            seats: account.seats,
            current_period_end: account.current_period_end,
            cancel_at_period_end: account.cancel_at_period_end,
            trial_ends_at: account.trial_ends_at,
            created_at: account.created_at,
        }
    }
}

impl From<AuditEntry> for AuditExport {
    fn from(entry: AuditEntry) -> Self {
        Self {
            panel: entry.panel,
            team_id: entry.tenant.map(TenantId::get),
            event: entry.event,
            resource: entry.resource,
            record: entry.record,
            changes: entry.changes.0,
            created_at: entry.created_at,
        }
    }
}

impl From<StoredNotification> for NotificationExport {
    fn from(stored: StoredNotification) -> Self {
        Self {
            panel: stored.panel,
            team_id: stored.tenant.map(TenantId::get),
            notification: stored.notification.0,
            read_at: stored.read_at,
            created_at: stored.created_at,
        }
    }
}

/// Reads what `user` owns, in the order [`super::deletion`] erases it.
///
/// # Errors
///
/// When a query fails.
pub async fn export_account(
    ex: &mut dyn toasty::Executor,
    user: uuid::Uuid,
) -> Result<AccountExport> {
    let account = User::get_by_id(&mut *ex, user).await?;
    let identities = Identity::filter(Identity::fields().user_id().eq(user))
        .exec(&mut *ex)
        .await?;
    let seats = Member::filter(Member::fields().user_id().eq(user))
        .exec(&mut *ex)
        .await?;
    let mut memberships = Vec::with_capacity(seats.len());
    let mut owned_teams = Vec::new();
    for seat in seats {
        let team = Team::get_by_id(&mut *ex, seat.team_id.get()).await?;
        memberships.push(MembershipExport {
            team_id: team.id,
            team_name: team.name.clone(),
            role: seat.role,
            joined_at: seat.created_at,
        });
        if seat.role == TeamRole::Owner {
            owned_teams.push(owned_team(&mut *ex, team).await?);
        }
    }
    let invitations = Invitation::filter(Invitation::fields().email().eq(&account.email))
        .exec(&mut *ex)
        .await?;
    let audit_entries =
        AuditEntry::filter(AuditEntry::fields().actor_id().eq(Some(user.to_string())))
            .exec(&mut *ex)
            .await?;
    let notifications =
        StoredNotification::filter(StoredNotification::fields().user_id().eq(user.to_string()))
            .exec(&mut *ex)
            .await?;
    Ok(AccountExport {
        exported_at: Timestamp::now(),
        user: account.into(),
        identities: by_date(identities, |identity| identity.created_at),
        memberships,
        owned_teams,
        invitations: invitations.into_iter().map(Into::into).collect(),
        audit_entries: by_date(audit_entries, |entry| entry.created_at),
        notifications: by_date(notifications, |stored| stored.created_at),
    })
}

/// What an owned team holds, each read naming the team: no row of another tenant can join it.
async fn owned_team(ex: &mut dyn toasty::Executor, team: Team) -> Result<TeamExport> {
    let tenant = team.tenant();
    let projects = Project::filter(Project::fields().team_id().eq(tenant))
        .exec(&mut *ex)
        .await?;
    let invitations = Invitation::filter(Invitation::fields().team_id().eq(tenant))
        .exec(&mut *ex)
        .await?;
    let billing_account = BillingAccount::filter(BillingAccount::fields().team_id().eq(tenant))
        .first()
        .exec(&mut *ex)
        .await?;
    Ok(TeamExport {
        id: team.id,
        name: team.name,
        created_at: team.created_at,
        projects: by_date(projects, |project| project.created_at),
        invitations: invitations.into_iter().map(Into::into).collect(),
        billing_account: billing_account.map(Into::into),
    })
}

/// Oldest first, which no query here promises.
fn by_date<T, E: From<T>>(mut rows: Vec<T>, at: impl Fn(&T) -> Timestamp) -> Vec<E> {
    rows.sort_by_key(|row| at(row));
    rows.into_iter().map(Into::into).collect()
}

/// `GET /app/settings/profile/export`: the signed-in account's file. A read, so it takes no CSRF
/// token; `no-store` keeps it out of shared caches and the browser's.
#[route(GET "/app/settings/profile/export")]
async fn download(cx: &Cx) -> Result<(HeaderMap, String)> {
    let signed = auth::require_user::<SignedIn>(cx)?;
    let export = export_account(&mut db(cx)?, signed.user.id).await?;
    let body = serde_json::to_string_pretty(&export)
        .map_err(|error| topcoat::Error::msg(error.to_string()))?;
    let filename = format!(
        "attachment; filename=\"account-{}.json\"",
        export.exported_at.strftime("%Y-%m-%d")
    );
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&filename).map_err(|error| topcoat::Error::msg(error.to_string()))?,
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((headers, body))
}

/// The profile page's button: a header action renders where the delete button does, and lands on
/// the download.
pub struct ExportAccount;

impl HeaderAction for ExportAccount {
    type Input = ();
    const NAME: &'static str = "export-account";

    async fn run(
        _cx: &Cx,
        (): (),
        _ex: &mut dyn toasty::Executor,
    ) -> std::result::Result<Outcome, WriteError<()>> {
        Ok(Outcome::done().redirect(EXPORT_URL))
    }
}

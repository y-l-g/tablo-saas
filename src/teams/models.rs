//! The team tables.

use jiff::Timestamp;
use tablo::TenantId;
use toasty::Deferred;

use crate::accounts::User;

/// A customer organisation: the tenant, and the unit a subscription belongs to.
#[derive(Debug, Clone, toasty::Model, tablo::ModelColumns)]
pub struct Team {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    pub name: String,
    #[default(Timestamp::now())]
    pub created_at: Timestamp,
    #[update(Timestamp::now())]
    pub updated_at: Timestamp,
}

impl Team {
    pub fn tenant(&self) -> TenantId {
        self.id.into()
    }
}

/// One user's seat on one team.
#[derive(Debug, Clone, toasty::Model, tablo::ModelColumns)]
#[unique(team_id, user_id)]
pub struct Member {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[index]
    pub team_id: TenantId,
    #[index]
    pub user_id: uuid::Uuid,
    #[belongs_to(key = user_id, references = id)]
    pub user: Deferred<User>,
    pub role: TeamRole,
    #[default(Timestamp::now())]
    pub created_at: Timestamp,
}

/// An address asked to join a team, until it accepts, declines or expires.
#[derive(Debug, Clone, toasty::Model, tablo::ModelColumns)]
#[unique(team_id, email)]
pub struct Invitation {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    #[index]
    pub team_id: TenantId,
    /// The team's name when invited, which the invitee reads without being a member.
    pub team_name: String,
    /// Lowercase, as `normalize_email` spells it.
    #[index]
    pub email: String,
    pub role: TeamRole,
    pub invited_by: String,
    pub expires_at: Timestamp,
    #[default(Timestamp::now())]
    pub created_at: Timestamp,
}

/// What a member may do on their team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, toasty::Embed, tablo::Options)]
#[serde(rename_all = "snake_case")]
pub enum TeamRole {
    /// Everything, billing and deleting the team included. A team has exactly one.
    #[option(color = "warning")]
    Owner,
    /// Manages members and team settings.
    #[option(color = "info")]
    Admin,
    /// Uses the product.
    #[option(color = "gray")]
    Member,
}

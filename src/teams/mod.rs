//! Teams, the tenants of the `app` panel: who belongs, what each member may do, and how people
//! join.
//!
//! Every account owns a personal team from sign-up, so a session always acts for a team and no
//! page handles "no tenant". An invitation names an address and is accepted from the invited
//! account, on `/app/incoming-invitations`: proving you read the address is the verified account
//! itself, so no secret token travels by mail.

mod access;
mod invitations;
mod mail;
mod members;
mod models;
mod registrar;

/// The actions on team pages, for tests and the back office.
pub mod actions {
    pub use super::invitations::{
        AcceptInvitation, DeclineInvitation, InviteMember, RevokeInvitation,
    };
    pub use super::members::{ChangeRole, LeaveTeam, RemoveMember, TransferOwnership};
}

pub use access::{current_entitlements, current_role, may_manage_members, role_in};
pub use invitations::{IncomingInvitationResource, InvitationResource};
pub use mail::{PruneInvitations, SendInvitation};
pub use members::MemberResource;
pub use models::{Invitation, Member, Team, TeamRole};
pub use registrar::{NewTeam, Teams, create_team};

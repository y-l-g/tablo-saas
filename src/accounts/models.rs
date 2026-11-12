//! The people who sign in.

use jiff::Timestamp;
use toasty::Deferred;
use topcoat::l10n::Locale;

pub use tablo::auth::normalize_email;

use crate::teams::Member;

/// A person with an account. A row without a staff role is a customer.
#[derive(Debug, Clone, toasty::Model, tablo::PanelModel)]
pub struct User {
    #[key]
    #[auto]
    pub id: uuid::Uuid,
    pub name: String,
    /// Lowercase, as [`normalize_email`] spells it.
    #[unique]
    pub email: String,
    /// When the address was confirmed, through a mailed link.
    pub email_verified_at: Option<Timestamp>,
    /// An Argon2id hash in PHC string format, which no column, export or audit entry spells.
    #[columns(skip)]
    pub password: String,
    /// What the account does as staff, which opens the back office. Sign-up writes none: only
    /// the `staff` command hands roles out, so no page can make an administrator.
    pub staff_role: Option<StaffRole>,
    /// When the `staff` command last granted the role. A second factor the account enrolled
    /// afterwards was added by whoever held its password then, so acting as a customer does not
    /// trust it: see `support::may_act_as`.
    pub staff_since: Option<Timestamp>,
    /// The language picked from a panel's language menu, such as `fr`. None until a pick: pages
    /// then follow the browser, and mail the panels' default.
    pub locale: Option<String>,
    #[default(Timestamp::now())]
    pub created_at: Timestamp,
    #[update(Timestamp::now())]
    pub updated_at: Timestamp,
    #[has_many(pair = user)]
    pub memberships: Deferred<Vec<Member>>,
}

impl User {
    /// The language the user picked, which their pages and mail read.
    pub fn language(&self) -> Option<Locale> {
        self.locale.as_deref()?.parse().ok()
    }
}

/// What a member of staff may do in the back office.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, toasty::Embed, tablo::Options)]
#[serde(rename_all = "snake_case")]
pub enum StaffRole {
    /// Reads and changes everything.
    #[option(color = "danger")]
    Administrator,
    /// Reads customers and their billing, to answer them.
    #[option(color = "info")]
    Support,
}

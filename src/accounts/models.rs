//! The people who sign in.

use jiff::Timestamp;
use toasty::Deferred;

use crate::teams::Member;

/// A person with an account. A row without a staff role is a customer.
#[derive(Debug, Clone, toasty::Model, tablo::ModelColumns)]
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
    /// An Argon2id hash in PHC string format.
    pub password: String,
    /// What the account does as staff, which opens the back office. Sign-up writes none: only
    /// the `staff` command hands roles out, so no page can make an administrator.
    pub staff_role: Option<StaffRole>,
    #[default(Timestamp::now())]
    pub created_at: Timestamp,
    #[update(Timestamp::now())]
    pub updated_at: Timestamp,
    #[has_many(pair = user)]
    pub memberships: Deferred<Vec<Member>>,
}

/// What a member of staff may do in the back office.
#[derive(Debug, Clone, Copy, PartialEq, Eq, toasty::Embed, tablo::Options)]
pub enum StaffRole {
    /// Reads and changes everything.
    #[option(color = "danger")]
    Administrator,
    /// Reads customers and their billing, to answer them.
    #[option(color = "info")]
    Support,
}

/// The one spelling an address is stored and looked up in.
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::normalize_email;

    #[test]
    fn an_address_has_one_spelling() {
        assert_eq!(normalize_email("  Ada@Example.COM "), "ada@example.com");
    }
}

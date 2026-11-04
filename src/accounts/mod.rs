//! Who signs in: users, read through Tablo's authenticator, with sign-up, password reset, email
//! verification, the profile and two-factor sign-in.
//!
//! An address is stored lowercase, so `Ada@Example.com` and `ada@example.com` are one account.

mod models;
mod profile;
mod sign_in;

pub use models::{StaffRole, User, normalize_email};
pub use profile::{AccountResetter, AccountVerifier};
pub use sign_in::{Accounts, Seat, SignUp, SignedIn, UserResource};

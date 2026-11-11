//! Who signs in: users, read through Tablo's [`PasswordAuth`](tablo::PasswordAuth), with sign-up, password reset, email
//! verification, the profile, two-factor sign-in and signing in with Google.
//!
//! An address is stored lowercase, so `Ada@Example.com` and `ada@example.com` are one account.

mod identity;
mod models;
mod sign_in;

pub use identity::Identity;
pub use models::{StaffRole, User, normalize_email};
pub use sign_in::{Accounts, Seat, SignUp, SignedIn, is_impersonated, password_auth};

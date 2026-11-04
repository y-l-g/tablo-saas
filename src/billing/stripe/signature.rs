//! Proving a webhook came from Stripe.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

/// How old a signature may be, which limits replaying a captured request.
const TOLERANCE_SECONDS: i64 = 300;

/// Why a webhook's signature was refused.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignatureError {
    #[error("the signature header is malformed")]
    Malformed,
    #[error("the signature is too old")]
    Expired,
    #[error("no signature matches")]
    Mismatch,
}

/// Verifies a `Stripe-Signature` header over the raw `body`.
///
/// The header may carry several `v1` signatures while a secret rolls, and one match is enough.
/// The comparison is `Mac::verify_slice`'s, which takes constant time.
pub fn verify_signature(
    header: &str,
    body: &[u8],
    secret: &str,
    now_seconds: i64,
) -> Result<(), SignatureError> {
    let mut timestamp = None;
    let mut signatures = Vec::new();
    for part in header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", value)) => timestamp = value.parse::<i64>().ok(),
            Some(("v1", value)) => signatures.push(value),
            _ => {}
        }
    }
    let timestamp = timestamp.ok_or(SignatureError::Malformed)?;
    if signatures.is_empty() {
        return Err(SignatureError::Malformed);
    }
    if (now_seconds - timestamp).abs() > TOLERANCE_SECONDS {
        return Err(SignatureError::Expired);
    }
    let mut mac = keyed(secret);
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    let matches = signatures.into_iter().any(|signature| {
        hex::decode(signature).is_ok_and(|bytes| mac.clone().verify_slice(&bytes).is_ok())
    });
    if matches {
        Ok(())
    } else {
        Err(SignatureError::Mismatch)
    }
}

fn keyed(secret: &str) -> Hmac<Sha256> {
    Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes a key of any length")
}

/// Signs `body` as Stripe would, for tests of the webhook route.
pub fn sign(body: &[u8], secret: &str, timestamp: i64) -> String {
    let mut mac = keyed(secret);
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    format!(
        "t={timestamp},v1={}",
        hex::encode(mac.finalize().into_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "whsec_test";
    const NOW: i64 = 1_800_000_000;

    #[test]
    fn a_signature_from_stripe_verifies() {
        let header = sign(b"{}", SECRET, NOW);
        assert_eq!(verify_signature(&header, b"{}", SECRET, NOW + 10), Ok(()));
    }

    #[test]
    fn a_changed_body_or_secret_does_not() {
        let header = sign(b"{}", SECRET, NOW);
        assert_eq!(
            verify_signature(&header, b"{ }", SECRET, NOW),
            Err(SignatureError::Mismatch)
        );
        assert_eq!(
            verify_signature(&header, b"{}", "whsec_other", NOW),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn a_replay_outside_the_tolerance_is_refused() {
        let header = sign(b"{}", SECRET, NOW);
        assert_eq!(
            verify_signature(&header, b"{}", SECRET, NOW + 301),
            Err(SignatureError::Expired)
        );
    }

    #[test]
    fn one_good_signature_among_several_is_enough() {
        let good = sign(b"{}", SECRET, NOW);
        let value = good.split("v1=").nth(1).expect("signed");
        let header = format!("t={NOW},v1=00ff,v1={value},v0=abc");
        assert_eq!(verify_signature(&header, b"{}", SECRET, NOW), Ok(()));
    }

    #[test]
    fn a_header_without_a_time_or_a_signature_is_malformed() {
        assert_eq!(
            verify_signature("v1=abc", b"{}", SECRET, NOW),
            Err(SignatureError::Malformed)
        );
        assert_eq!(
            verify_signature("t=1", b"{}", SECRET, 1),
            Err(SignatureError::Malformed)
        );
    }
}

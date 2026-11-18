//! Auth mail made durable: [`Outbox`] queues each letter Tablo's auth sends, inside the
//! transaction that records the letter's token, and [`SendMail`] sends it from the worker.
//!
//! Sent straight from the request, a crash or an SMTP outage would lose the mail; queued, the
//! letter commits with its token or not at all, and the worker retries it like any job.
//!
//! A letter carries a live link or code, and a job's payload lives in the database and its
//! backups, so the payload is sealed: AES-256-GCM under a key derived from `APP_COOKIE_KEY`.
//! Rotating that key strands the mail still queued, and the person asks for it again. A web
//! process and a separate worker share the key: in development, where it is generated at start,
//! set it or run `serve` alone.

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use hmac::{Hmac, Mac};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tablo::{AuthLetter, AuthMailDelivery};
use topcoat::{
    context::{Cx, context},
    cookie::Key,
};

use super::{Job, enqueue};

/// What the seal's key is derived for, so it never equals a key that seals cookies.
const PURPOSE: &[u8] = b"tablo-saas/jobs/send-mail";

/// The bytes of a nonce at the head of a sealed mail.
const NONCE_LEN: usize = 12;

/// Seals and opens queued mail. Someone with the database but not `APP_COOKIE_KEY` reads no link
/// in it.
#[derive(Clone)]
pub struct MailSeal {
    cipher: Aes256Gcm,
}

impl MailSeal {
    /// A seal with a key of its own, derived from `key`.
    pub fn new(key: &Key) -> Self {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key.encryption())
            .expect("HMAC accepts a key of any length");
        mac.update(PURPOSE);
        Self {
            cipher: Aes256Gcm::new(&mac.finalize().into_bytes()),
        }
    }

    fn seal(&self, plain: &[u8]) -> anyhow::Result<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let sealed = self
            .cipher
            .encrypt(&nonce, plain)
            .map_err(|_| anyhow::anyhow!("sealing a mail failed"))?;
        let mut bytes = nonce.to_vec();
        bytes.extend(sealed);
        Ok(hex::encode(bytes))
    }

    fn open(&self, sealed: &str) -> anyhow::Result<Vec<u8>> {
        let bytes = hex::decode(sealed)?;
        anyhow::ensure!(bytes.len() > NONCE_LEN, "a sealed mail is too short");
        let (nonce, body) = bytes.split_at(NONCE_LEN);
        self.cipher
            .decrypt(Nonce::from_slice(nonce), body)
            .map_err(|_| {
                anyhow::anyhow!("a queued mail does not open: was APP_COOKIE_KEY changed?")
            })
    }
}

/// Queues each auth letter, sealed, through the transaction Tablo hands it.
#[derive(Clone)]
pub struct Outbox {
    seal: MailSeal,
}

impl Outbox {
    /// Queues letters sealed by `seal`.
    pub fn new(seal: MailSeal) -> Self {
        Self { seal }
    }
}

impl AuthMailDelivery for Outbox {
    async fn deliver(
        &self,
        _cx: &Cx,
        ex: &mut dyn toasty::Executor,
        letter: AuthLetter,
    ) -> topcoat::Result<()> {
        let queued = async {
            let sealed = self.seal.seal(&serde_json::to_vec(&letter)?)?;
            enqueue(&SendMail { sealed }).exec(ex).await
        };
        // Refused, the token rolls back with the row: no link goes out that no mail carries.
        queued.await.map_err(|error| {
            topcoat::Error::msg(format!("queueing an auth mail failed: {error:#}"))
        })
    }
}

/// Sends a letter the [`Outbox`] queued, through the worker's transport.
#[derive(Serialize, Deserialize)]
pub struct SendMail {
    /// An [`AuthLetter`] as JSON, sealed.
    sealed: String,
}

impl Job for SendMail {
    const KIND: &'static str = "send-mail";
    /// About a quarter of an hour of backoff: a two-factor code is spent by then, and a link
    /// hours late reads as broken. The person asks again.
    const MAX_ATTEMPTS: u32 = 6;

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let letter: AuthLetter =
            serde_json::from_slice(&context::<MailSeal>(cx).open(&self.sealed)?)?;
        if letter.expires_at().is_some_and(|at| at <= Timestamp::now()) {
            tracing::warn!(kind = ?letter.kind(), "an auth mail expired in the queue: dropped");
            return Ok(());
        }
        let mail = letter
            .mail()
            .map_err(|error| anyhow::anyhow!("{error:#}"))?;
        topcoat::mail::send(cx, mail)
            .await
            .map_err(|error| anyhow::anyhow!("{error:#}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sealed_mail_hides_its_link_and_opens_under_its_key_alone() {
        let key = Key::generate();
        let seal = MailSeal::new(&key);
        let sealed = seal.seal(b"token=secret").expect("seal");
        assert!(!sealed.contains(&hex::encode(b"secret")));
        assert_eq!(seal.open(&sealed).expect("open"), b"token=secret");
        assert!(MailSeal::new(&Key::generate()).open(&sealed).is_err());

        let mut tampered = hex::decode(&sealed).expect("hex");
        *tampered.last_mut().expect("bytes") ^= 1;
        assert!(seal.open(&hex::encode(tampered)).is_err());
    }
}

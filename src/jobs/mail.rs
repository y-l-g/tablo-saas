//! Mail sent while answering a request, made durable: [`Outbox`] queues it and [`SendMail`] sends
//! it from the worker.
//!
//! Tablo sends its auth mail (verification, reset, email change, two-factor codes) while it
//! answers, most of it from a task spawned after the response. Sent from there, a crash or an SMTP
//! outage loses the mail; queued, the worker retries it like any job.
//!
//! An auth mail carries a live link or code, and a job's payload lives in the database and its
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
use tablo::db::db;
use topcoat::{
    context::{Cx, context},
    cookie::Key,
    mail::{Mail, Mailbox, Receipt, Transport, TransportFuture},
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

/// The app's transport inside a request: queues each mail it can carry and sends the rest, or any
/// mail the queue refuses, through `inner` at once.
#[derive(Clone)]
pub struct Outbox<T> {
    inner: T,
    seal: MailSeal,
}

impl<T> Outbox<T> {
    /// Queues through the request's database, sealed by `seal`; falls back to `inner`.
    pub fn new(inner: T, seal: MailSeal) -> Self {
        Self { inner, seal }
    }

    /// Queues `mail`, answering `false` for a mail a [`Letter`] cannot carry.
    async fn queue(&self, cx: &Cx, mail: &Mail) -> anyhow::Result<bool> {
        let Some(letter) = Letter::carry(mail) else {
            return Ok(false);
        };
        let sealed = self.seal.seal(&serde_json::to_vec(&letter)?)?;
        let mut handle = db(cx).map_err(|error| anyhow::anyhow!("{error:#}"))?;
        enqueue(&SendMail { sealed }).exec(&mut handle).await?;
        Ok(true)
    }
}

impl<T: Transport> Transport for Outbox<T> {
    fn send<'a>(&'a self, cx: &'a Cx, mail: Mail) -> TransportFuture<'a> {
        Box::pin(async move {
            match self.queue(cx, &mail).await {
                Ok(true) => Ok(Receipt::new("queued")),
                Ok(false) => self.inner.send(cx, mail).await,
                // A database that refuses the row may still leave SMTP up: one try beats none.
                Err(error) => {
                    tracing::warn!(error = %format!("{error:#}"), "queueing a mail failed: sending it now");
                    self.inner.send(cx, mail).await
                }
            }
        })
    }
}

/// Sends a mail the [`Outbox`] queued, through the worker's transport.
#[derive(Serialize, Deserialize)]
pub struct SendMail {
    /// A [`Letter`] as JSON, sealed.
    sealed: String,
}

impl Job for SendMail {
    const KIND: &'static str = "send-mail";
    /// About a quarter of an hour of backoff: a two-factor code is spent by then, and a link
    /// hours late reads as broken. The person asks again.
    const MAX_ATTEMPTS: u32 = 6;

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let letter: Letter = serde_json::from_slice(&context::<MailSeal>(cx).open(&self.sealed)?)?;
        topcoat::mail::send(cx, letter.into_mail()?)
            .await
            .map_err(|error| anyhow::anyhow!("{error:#}"))?;
        Ok(())
    }
}

/// A plain-text mail, which is what Tablo sends. Its date and Message-ID travel with it, so a mail
/// sent twice after a crash reads as one in a mail client.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Letter {
    from: Address,
    to: Vec<Address>,
    cc: Vec<Address>,
    bcc: Vec<Address>,
    reply_to: Vec<Address>,
    subject: String,
    text: String,
    date: Timestamp,
    message_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Address {
    name: Option<String>,
    address: String,
}

impl Letter {
    /// `mail` as a letter, or `None` when it has a part a letter drops: HTML, an attachment or a
    /// header of its own.
    fn carry(mail: &Mail) -> Option<Self> {
        let plain = mail.html().is_none()
            && mail.attachments().is_empty()
            && mail.headers().is_empty()
            && mail.in_reply_to().is_none()
            && mail.references().is_none();
        let addresses = |list: &[Mailbox]| list.iter().map(Address::from).collect();
        Some(Self {
            from: Address::from(mail.from()),
            to: addresses(mail.to()),
            cc: addresses(mail.cc()),
            bcc: addresses(mail.bcc()),
            reply_to: addresses(mail.reply_to()),
            subject: mail.subject().to_string(),
            text: mail.text().filter(|_| plain)?.to_string(),
            date: Timestamp::try_from(mail.date()).ok()?,
            message_id: mail.message_id().to_string(),
        })
    }

    fn into_mail(self) -> anyhow::Result<Mail> {
        let mailboxes = |list: Vec<Address>| {
            list.into_iter()
                .map(Address::into_mailbox)
                .collect::<anyhow::Result<Vec<_>>>()
        };
        Ok(Mail::builder()
            .from(self.from.into_mailbox()?)
            .to(mailboxes(self.to)?)
            .cc(mailboxes(self.cc)?)
            .bcc(mailboxes(self.bcc)?)
            .reply_to(mailboxes(self.reply_to)?)
            .subject(self.subject)
            .text(self.text)
            .date(self.date.into())
            .message_id(self.message_id)
            .build()?)
    }
}

impl From<&Mailbox> for Address {
    fn from(mailbox: &Mailbox) -> Self {
        Self {
            name: mailbox.name().map(str::to_string),
            address: mailbox.address().to_string(),
        }
    }
}

impl Address {
    fn into_mailbox(self) -> anyhow::Result<Mailbox> {
        let mailbox = match self.name {
            Some(name) => Mailbox::named(name, &self.address),
            None => Mailbox::new(&self.address),
        };
        mailbox.map_err(|error| anyhow::anyhow!("{error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mail() -> Mail {
        Mail::builder()
            .from(Mailbox::named("Tablo", "noreply@example.com").expect("address"))
            .to([Mailbox::new("ada@example.com").expect("address")])
            .bcc([Mailbox::new("audit@example.com").expect("address")])
            .subject("Reset your password")
            .text("https://example.com/app/reset?token=secret")
            .build()
            .expect("a mail")
    }

    #[test]
    fn a_letter_rebuilds_the_mail_it_carried() {
        let original = mail();
        let letter = Letter::carry(&original).expect("plain text is carried");
        let json = serde_json::to_vec(&letter).expect("serialize");
        let rebuilt = serde_json::from_slice::<Letter>(&json)
            .expect("deserialize")
            .into_mail()
            .expect("rebuild");
        assert_eq!(rebuilt.from(), original.from());
        assert_eq!(rebuilt.to(), original.to());
        assert_eq!(rebuilt.bcc(), original.bcc());
        assert_eq!(rebuilt.subject(), original.subject());
        assert_eq!(rebuilt.text(), original.text());
        assert_eq!(rebuilt.message_id(), original.message_id());
        assert_eq!(
            Timestamp::try_from(rebuilt.date()).expect("date"),
            Timestamp::try_from(original.date()).expect("date"),
        );
    }

    #[test]
    fn a_mail_with_a_custom_header_is_not_carried() {
        let mail = Mail::builder()
            .from(Mailbox::new("noreply@example.com").expect("address"))
            .to([Mailbox::new("ada@example.com").expect("address")])
            .subject("News")
            .text("Hello")
            .headers([("List-Unsubscribe", "<https://example.com/unsubscribe>")])
            .build()
            .expect("a mail");
        assert!(Letter::carry(&mail).is_none());
    }

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

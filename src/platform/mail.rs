//! How mail leaves the app: through an SMTP server, or into the log during development.

use std::{sync::Arc, time::Duration};

use anyhow::Context;
use topcoat::{
    context::{Cx, context},
    mail::{Mail, Mailbox, Receipt, SmtpTransport, Transport, TransportFuture, send},
    view::view,
};

use super::config::{Settings, Smtp};

/// How many times a send is tried before it fails. A blip costs seconds here rather than the
/// half-minute a job waits before its retry, and a mail sent outside the queue gets no other.
const ATTEMPTS: u32 = 3;

/// The app's transport. Cloning shares one connection pool.
#[derive(Clone)]
pub struct Postbox {
    smtp: Option<Arc<SmtpTransport>>,
}

impl Postbox {
    /// Sends through `smtp`. Without one, a mail is logged instead, which is what lets a
    /// development server sign up with no mail server: `Config` refuses that in production.
    pub fn new(smtp: Option<&Smtp>) -> anyhow::Result<Self> {
        Ok(Self {
            smtp: smtp.map(transport).transpose()?.map(Arc::new),
        })
    }
}

impl Transport for Postbox {
    fn send<'a>(&'a self, cx: &'a Cx, mail: Mail) -> TransportFuture<'a> {
        Box::pin(async move {
            let Some(smtp) = &self.smtp else {
                tracing::info!(to = ?mail.to(), subject = mail.subject(), "no SMTP server: mail logged");
                return Ok(Receipt::new("logged"));
            };
            let mut attempt = 1;
            loop {
                match smtp.send(cx, mail.clone()).await {
                    Ok(receipt) => return Ok(receipt),
                    Err(error) if attempt == ATTEMPTS => {
                        return Err(topcoat::Error::msg(format!("{error:#}")));
                    }
                    Err(error) => {
                        tracing::warn!(attempt, error = %format!("{error:#}"), "sending mail failed");
                    }
                }
                tokio::time::sleep(Duration::from_secs(u64::from(attempt) * 2)).await;
                attempt += 1;
            }
        })
    }
}

/// What a mail to one person says.
pub struct Message<'a> {
    pub to: &'a str,
    pub subject: &'a str,
    pub title: &'a str,
    pub body: &'a str,
    pub action: &'a str,
    pub url: &'a str,
}

/// Sends a mail with a title, a paragraph and one button: the shape of every mail the app sends.
///
/// Inline styles alone: a mail client reads no stylesheet. The caller is a job, and the error
/// sends it round again.
pub async fn send_message(cx: &Cx, message: Message<'_>) -> anyhow::Result<()> {
    let Message {
        to,
        subject,
        title,
        body,
        action,
        url,
    } = message;
    let from = context::<Settings>(cx).mail_from.clone();
    let to = Mailbox::new(to).map_err(|error| anyhow::anyhow!("{error:#}"))?;
    let html = view! {
        <div style="font-family: system-ui, sans-serif; max-width: 32rem; margin: 0 auto; color: #1f2430;">
            <h1 style="font-size: 1.25rem;">(title)</h1>
            <p style="line-height: 1.5;">(body)</p>
            <p>
                <a href=(url) style="display: inline-block; background: #594ddc; color: #fff; padding: 0.625rem 1rem; border-radius: 0.5rem; text-decoration: none;">
                    (action)
                </a>
            </p>
        </div>
    };
    let mail = Mail::builder()
        .from(from)
        .to([to])
        .subject(subject)
        .html(cx, html)
        .await
        .map_err(|error| anyhow::anyhow!("{error:#}"))?
        .build()?;
    send(cx, mail)
        .await
        .map_err(|error| anyhow::anyhow!("{error:#}"))?;
    Ok(())
}

/// A transport to `smtp`: implicit TLS on port 465, STARTTLS on any other, and plain text on the
/// loopback a development catcher listens on.
fn transport(smtp: &Smtp) -> anyhow::Result<SmtpTransport> {
    let builder = if smtp.port == 465 {
        SmtpTransport::relay(&smtp.host)
    } else if matches!(smtp.host.as_str(), "127.0.0.1" | "localhost") {
        Ok(SmtpTransport::unencrypted(&smtp.host))
    } else {
        SmtpTransport::starttls(&smtp.host)
    }
    .context("SMTP_HOST names an SMTP server")?
    .port(smtp.port);
    Ok(match &smtp.credentials {
        Some((username, password)) => builder.credentials(username.clone(), password.clone()),
        None => builder,
    }
    .build())
}

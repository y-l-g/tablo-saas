//! How mail leaves the app: through an SMTP server, or into the log during development.

use std::{sync::Arc, time::Duration};

use anyhow::Context;
use topcoat::{
    context::Cx,
    mail::{Mail, Receipt, SmtpTransport, Transport, TransportFuture},
};

use super::config::Smtp;

/// How many times a send is tried before it fails. Tablo sends its auth mail once, after the
/// response, so a passing SMTP outage would otherwise cost a person their reset link.
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

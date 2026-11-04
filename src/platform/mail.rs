//! How mail leaves the app: through an SMTP server, or into the log during development.

use anyhow::Context;
use topcoat::{
    context::Cx,
    mail::{Mail, Receipt, SmtpTransport, Transport, TransportFuture},
};

use super::config::Smtp;

/// The app's transport.
pub struct Postbox {
    smtp: Option<SmtpTransport>,
}

impl Postbox {
    /// Sends through `smtp`. Without one, a mail is logged instead, links included, which is
    /// what lets a development server sign up with no mail server: `Config` refuses that in
    /// production.
    pub fn new(smtp: Option<&Smtp>) -> anyhow::Result<Self> {
        Ok(Self {
            smtp: smtp.map(transport).transpose()?,
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
            smtp.send(cx, mail)
                .await
                .map_err(|error| topcoat::Error::msg(format!("{error:#}")))
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

//! The mail an invitation sends.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tablo::db::db;
use topcoat::context::{Cx, context};

use super::Invitation;
use crate::jobs::Job;
use crate::platform::{
    config::Settings,
    mail::{Message, send_message},
    words::job_text,
};

/// Mails the invitee a link to their invitations. It names the row, not its contents, so a revoked
/// invitation sends nothing and no address sits in the queue.
#[derive(Serialize, Deserialize)]
pub struct SendInvitation {
    pub invitation: uuid::Uuid,
}

impl Job for SendInvitation {
    const KIND: &'static str = "send-invitation";

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let mut handle = db(cx).map_err(|error| anyhow::anyhow!("{error:#}"))?;
        let Some(invitation) = Invitation::filter_by_id(self.invitation)
            .first()
            .exec(&mut handle)
            .await?
        else {
            return Ok(());
        };
        let team = invitation.team_name.as_str();
        let inviter = invitation.invited_by.as_str();
        let url = format!(
            "{}/app/incoming-invitations",
            context::<Settings>(cx).base_url
        );
        send_message(
            cx,
            Message {
                to: &invitation.email,
                subject: &job_text("mail-invitation-subject", &[("team", team.into())])?,
                title: &job_text("mail-invitation-title", &[("team", team.into())])?,
                body: &job_text(
                    "mail-invitation-body",
                    &[("inviter", inviter.into()), ("team", team.into())],
                )?,
                action: &job_text("mail-invitation-action", &[])?,
                url: &url,
            },
        )
        .await
    }
}

/// Deletes invitations past their date: nobody can accept them, and the address has no business
/// staying on file.
#[derive(Clone, Serialize, Deserialize)]
pub struct PruneInvitations;

impl Job for PruneInvitations {
    const KIND: &'static str = "prune-invitations";

    async fn run(self, cx: &Cx) -> anyhow::Result<()> {
        let mut handle = db(cx).map_err(|error| anyhow::anyhow!("{error:#}"))?;
        Invitation::filter(Invitation::fields().expires_at().le(Timestamp::now()))
            .delete()
            .exec(&mut handle)
            .await?;
        Ok(())
    }
}

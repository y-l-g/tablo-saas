//! The `admin` panel: where staff see customers and repair what went wrong. Only an account with a
//! [`StaffRole`] gets in, with a second factor, and every write lands in the back office's audit log.
//! Support reads; an administrator also acts, and reads who acted as a customer in the app panel.

use jiff::Timestamp;
use tablo::{AuditEntry, auth, prelude::*};
use topcoat::{context::Cx, icon::iconify::iconify_icon};

use crate::accounts::{SignedIn, StaffRole, User};
use crate::audit::IMPERSONATION_EVENTS;
use crate::billing::{BillingAccount, StripeEvent, jobs::RefreshCustomer};
use crate::jobs::{JobRow, JobState, enqueue};
use crate::teams::Team;

fn role(cx: &Cx) -> Option<StaffRole> {
    auth::user::<SignedIn>(cx).and_then(|signed| signed.user.staff_role)
}

fn is_staff(cx: &Cx) -> bool {
    role(cx).is_some()
}

fn is_administrator(cx: &Cx) -> bool {
    role(cx) == Some(StaffRole::Administrator)
}

/// Every account.
pub struct AccountResource;

impl Resource for AccountResource {
    type Model = User;
    type Form = NoForm<User>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("accounts")
            .icon(iconify_icon!("lucide:users"))
            .tenancy(Tenancy::none())
            .record_title(lens!(User.email))
            .policy(ReadOnly.and(when(is_staff)))
            .without_page(ResourcePage::View)
            .table(
                Table::new((
                    TextColumn::new(lens!(User.name)).searchable().sortable(),
                    TextColumn::new(lens!(User.email)).searchable().sortable(),
                    TextColumn::new(lens!(User.staff_role)).badge(),
                    DateColumn::new(lens!(User.created_at)).sortable(),
                ))
                .filters((
                    // Every account of one company: `?f.email=acme.com&f.email.op=ends`.
                    TextFilter::new(lens!(User.email)).matching([
                        TextMatch::EndsWith,
                        TextMatch::Contains,
                        TextMatch::Is,
                    ]),
                    PresenceFilter::new(lens!(User.staff_role)).labels(
                        tablo::text!("filter-staff"),
                        tablo::text!("filter-customers"),
                    ),
                    PresenceFilter::new(lens!(User.email_verified_at)).labels(
                        tablo::text!("filter-verified"),
                        tablo::text!("filter-unverified"),
                    ),
                ))
                .default_sort_desc(lens!(User.created_at)),
            )
    }
}

/// Every time staff started or stopped acting as a customer. The app panel's audit log files each
/// entry under a team, the actor's on the way in and the customer's on the way out, so neither
/// team's trail tells the whole story, and the back office's own trail lists `/admin` alone.
pub struct ImpersonationResource;

/// The app panel's prefix, as its audit entries name it.
const APP_PANEL: &str = "/app";

impl Resource for ImpersonationResource {
    type Model = AuditEntry;
    type Form = NoForm<AuditEntry>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("impersonations")
            .label(tablo::text!("impersonation-entry"))
            .plural_label(tablo::text!("impersonation-log"))
            .icon(iconify_icon!("lucide:shield-check"))
            .tenancy(Tenancy::none())
            .policy(ReadOnly.and(when(is_administrator)))
            .without_page(ResourcePage::View)
            .without_global_search()
            .query(
                AuditEntry::fields().panel().eq(APP_PANEL.to_string()).and(
                    AuditEntry::fields()
                        .event()
                        .in_list(IMPERSONATION_EVENTS.map(String::from)),
                ),
            )
            .table(
                Table::new((
                    DateColumn::new(lens!(AuditEntry.created_at)).sortable(),
                    TextColumn::new(lens!(AuditEntry.actor))
                        .label(tablo::text!("impersonation-staff"))
                        .searchable(),
                    TextColumn::new(lens!(AuditEntry.event))
                        .label(tablo::text!("impersonation-event"))
                        .format(|event: &String| {
                            if event == "impersonate" {
                                tablo::text!("impersonation-started")
                            } else {
                                tablo::text!("impersonation-stopped")
                            }
                        })
                        .badge(),
                    ComputedColumn::new(tablo::text!("impersonation-until"), ends_by),
                    ComputedColumn::new(tablo::text!("impersonation-customer"), customer_named),
                    TextColumn::new(lens!(AuditEntry.record))
                        .label(tablo::text!("impersonation-customer-id"))
                        .searchable(),
                ))
                .default_sort_desc(lens!(AuditEntry.created_at)),
            )
    }
}

/// The customer an entry names, as they were called then: Tablo records them as the `user`
/// change's new value. A session that ran out is stopped without the name, and Tablo writes the
/// id in its place, which the next column already shows.
fn customer_named(entry: &AuditEntry) -> String {
    entry
        .changes
        .0
        .iter()
        .find(|change| change.field == "user")
        .and_then(|change| change.after.clone())
        .filter(|name| entry.record.as_deref() != Some(name.as_str()))
        .unwrap_or_else(|| "—".to_string())
}

/// When a start was due to end at the latest, as Tablo records it in the `until` change: a start
/// with no stop after it, whose session simply expired, ended then. A stop has none.
fn ends_by(entry: &AuditEntry) -> String {
    entry
        .changes
        .0
        .iter()
        .find(|change| change.field == "until")
        .and_then(|change| change.after.as_deref()?.parse::<Timestamp>().ok())
        .map_or_else(
            || "—".to_string(),
            |until| until.strftime("%Y-%m-%d %H:%M UTC").to_string(),
        )
}

/// Every team.
pub struct CustomerResource;

impl Resource for CustomerResource {
    type Model = Team;
    type Form = NoForm<Team>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("customers")
            .icon(iconify_icon!("lucide:building"))
            .tenancy(Tenancy::none())
            .record_title(lens!(Team.name))
            .policy(ReadOnly.and(when(is_staff)))
            .without_page(ResourcePage::View)
            .table(
                Table::new((
                    TextColumn::new(lens!(Team.name)).searchable().sortable(),
                    DateColumn::new(lens!(Team.created_at)).sortable(),
                ))
                .default_sort_desc(lens!(Team.created_at)),
            )
    }
}

/// Every subscription mirror.
pub struct SubscriptionResource;

impl Resource for SubscriptionResource {
    type Model = BillingAccount;
    type Form = NoForm<BillingAccount>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("subscriptions")
            .icon(iconify_icon!("lucide:credit-card"))
            .tenancy(Tenancy::none())
            .record_title(lens!(BillingAccount.customer_id))
            .policy(
                Abilities::<BillingAccount>::new()
                    .view_any(is_staff)
                    .view(|cx, _| is_staff(cx))
                    .run::<Resync, _>(|cx, _| is_administrator(cx)),
            )
            .without_page(ResourcePage::View)
            .action::<Resync>()
            .table(
                Table::new((
                    TextColumn::new(lens!(BillingAccount.customer_id)).searchable(),
                    TextColumn::new(lens!(BillingAccount.plan)).badge(),
                    TextColumn::new(lens!(BillingAccount.status)).badge(),
                    TextColumn::new(lens!(BillingAccount.seats)),
                    DateColumn::new(lens!(BillingAccount.current_period_end)).sortable(),
                    DateColumn::new(lens!(BillingAccount.synced_at)).sortable(),
                ))
                .filters((
                    SelectFilter::of(lens!(BillingAccount.status)),
                    SelectFilter::of(lens!(BillingAccount.plan)),
                ))
                .default_sort_desc(lens!(BillingAccount.created_at)),
            )
    }
}

/// Queues a refresh from Stripe.
pub struct Resync;

impl Action<SubscriptionResource> for Resync {
    type Input = ();
    const NAME: &'static str = "resync";

    fn tooltip() -> Option<Text> {
        Some(tablo::text!("hint-resync"))
    }

    async fn run(
        _cx: &Cx,
        accounts: &[BillingAccount],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<std::convert::Infallible>> {
        for account in accounts {
            enqueue(&RefreshCustomer {
                customer: account.customer_id.clone(),
            })
            .dedupe(format!("refresh:{}", account.customer_id))
            .exec(&mut *ex)
            .await
            .map_err(|error| topcoat::Error::msg(format!("{error:#}")))?;
        }
        Ok(Outcome::done())
    }
}

/// Every Stripe event in the inbox.
pub struct WebhookResource;

impl Resource for WebhookResource {
    type Model = StripeEvent;
    type Form = NoForm<StripeEvent>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("webhooks")
            .icon(iconify_icon!("lucide:zap"))
            .tenancy(Tenancy::none())
            .policy(ReadOnly.and(when(is_staff)))
            .without_page(ResourcePage::View)
            .without_global_search()
            .table(
                Table::new((
                    DateColumn::new(lens!(StripeEvent.received_at)).sortable(),
                    TextColumn::new(lens!(StripeEvent.kind)).searchable(),
                    TextColumn::new(lens!(StripeEvent.customer_id)).searchable(),
                    DateColumn::new(lens!(StripeEvent.processed_at)).sortable(),
                ))
                .filters(
                    (PresenceFilter::new(lens!(StripeEvent.processed_at)).labels(
                        tablo::text!("filter-processed"),
                        tablo::text!("filter-pending"),
                    ),),
                )
                .default_sort_desc(lens!(StripeEvent.received_at)),
            )
    }
}

/// The job queue, with the dead list a person works through.
pub struct JobResource;

impl Resource for JobResource {
    type Model = JobRow;
    type Form = NoForm<JobRow>;

    fn declare() -> ResourceDef<Self> {
        ResourceDef::new()
            .slug("jobs")
            .icon(iconify_icon!("lucide:list-checks"))
            .tenancy(Tenancy::none())
            .navigation_badge(JobRow::fields().state().eq(JobState::Dead))
            .policy(
                Abilities::<JobRow>::new()
                    .view_any(is_staff)
                    .view(|cx, _| is_staff(cx))
                    .run::<RetryJob, _>(|cx, _| is_administrator(cx)),
            )
            .without_page(ResourcePage::View)
            .without_global_search()
            .action::<RetryJob>()
            .table(
                Table::new((
                    TextColumn::new(lens!(JobRow.kind)).searchable(),
                    TextColumn::new(lens!(JobRow.state)).badge(),
                    TextColumn::new(lens!(JobRow.attempts)),
                    DateColumn::new(lens!(JobRow.run_at)).sortable(),
                    TextColumn::new(lens!(JobRow.last_error)).limit(80),
                ))
                .option_tabs(lens!(JobRow.state))
                .filters((NumberFilter::new(lens!(JobRow.attempts)).range(),))
                .default_sort_desc(lens!(JobRow.created_at)),
            )
    }
}

/// Gives a dead job its attempts back.
pub struct RetryJob;

impl Action<JobResource> for RetryJob {
    type Input = ();
    const NAME: &'static str = "retry";

    fn tooltip() -> Option<Text> {
        Some(tablo::text!("hint-retry"))
    }

    fn can_run(_cx: &Cx, job: &JobRow) -> bool {
        job.state == JobState::Dead
    }

    async fn run(
        _cx: &Cx,
        jobs: &[JobRow],
        (): (),
        ex: &mut dyn toasty::Executor,
    ) -> Result<Outcome, WriteError<std::convert::Infallible>> {
        for job in jobs.iter().filter(|job| job.state == JobState::Dead) {
            JobRow::filter_by_id(job.id)
                .update()
                .state(JobState::Queued)
                .attempts(0)
                .run_at(Timestamp::now())
                .exec(&mut *ex)
                .await?;
        }
        Ok(Outcome::done())
    }
}

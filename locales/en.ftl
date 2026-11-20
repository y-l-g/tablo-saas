# The app's own English messages. Tablo's catalog holds the rest; a label Tablo derives from a name
# falls back to its English spelling.

## Pages

page-home = Home

## Refusals

error-email-taken = This email address is already in use.
error-name-required = Enter your name.
error-email-invalid = Enter a valid email address.
error-team-name-required = Enter a team name.
error-owner-by-transfer = The owner changes only by transferring ownership.
error-already-member = This person is already on the team.
error-transfer-one = Transfer ownership to one member at a time.
error-no-user = Sign in again.
error-no-team = Pick a team first.

## Settings

settings-account = Account
settings-security = Security
settings-general = General
invitations-waiting = { $count ->
    [one] An invitation to join a team is waiting for you.
   *[other] { $count } invitations to join a team are waiting for you.
}

## Tablo's tenants, which this app calls teams

-tenant = { $form ->
   *[lower] team
    [upper] Team
    [upper-plural] Teams
    [indefinite] a team
}

## Teams

team-personal = { $name }'s team
flash-invitation-sent = Invitation sent to { $email }.

## Mail

mail-invitation-subject = Join { $team }
mail-invitation-title = You're invited to { $team }
mail-invitation-body = { $inviter } invited you to join { $team }. Sign in with this address to accept.
mail-invitation-action = See your invitation

## Billing

page-billing = Billing
plan-free = Free
plan-pro = Pro
plan-business = Business
billing-description = Your plan, seats and invoices.
billing-current = Current plan
billing-seats = { $used } seats
billing-seats-limited = { $used } of { $limit } seats
error-no-billing-account = This team has no billing account yet. Choose a plan first.
error-billing-unavailable = Billing is unavailable right now. Try again in a minute.
error-plan-invalid = Pick one of the plans on offer.
billing-off = Billing isn't configured on this server. Set the STRIPE_ variables to turn it on.
billing-per-seat = { $price } per seat per month
billing-monthly = Billed monthly
billing-yearly = Billed yearly, { $price } per seat
billing-invoices = Invoices
billing-invoice-open = Open
billing-trialing = Your trial runs until { $date }.
billing-ending = Your subscription ends on { $date }.
billing-grace = Your last payment failed. Update your card by { $date } to keep your plan.
billing-lapsed = Your subscription lapsed, so this team is on the Free plan. Nothing was deleted.
error-seat-limit = Your plan has no seat left. Upgrade to invite more people.
error-team-full = This team has no seat left. Ask its owner to upgrade.
mail-payment-failed-subject = Your payment failed
mail-payment-failed-body = We couldn't charge your card. We'll try again, but please update it to keep your plan.
mail-trial-ending-subject = Your trial ends soon
mail-trial-ending-body = Your trial ends in a few days. Your subscription starts then unless you cancel it.
mail-billing-action = Open billing

## Dashboard and projects

dashboard-seats = Seats
dashboard-projects = Projects
error-project-limit = Your plan has no project left. Upgrade to start another.


## Deletion

error-team-name-mismatch = Type the team's name exactly.
error-password-wrong = That password isn't right.
error-transfer-first = You own a team others use. Transfer it before deleting your account.

## Support

page-support = Support
support-description = Find a customer and act as them to see what they see. It lasts { $minutes } minutes at most, and the back office logs who acted as whom.
support-needs-factor = Acting as a customer needs a second factor here in the app, set up before an administrator granted your staff role. Set one up, then ask for the grant again.
support-teams = Teams
support-act-as = Act as this customer
support-act-as-one = Act as one customer at a time.
support-acting = You are now acting as this customer.
impersonation-entry = Impersonation
impersonation-log = Impersonations
impersonation-staff = Staff
impersonation-event = Event
impersonation-started = Started
impersonation-stopped = Stopped
impersonation-until = Ends at the latest
impersonation-customer = Customer
impersonation-customer-id = Customer id

## Website

site-nav = Site
site-pricing = Pricing
site-sign-in = Sign in
site-sign-up = Sign up
site-source = Source
site-terms = Terms
site-privacy = Privacy
site-kicker = A production SaaS template in Rust
site-title-1 = Fork the template.
site-title-2 = Replace one folder.
site-lead = Teams as tenants, per-seat Stripe subscriptions, a job queue in Postgres, account security and a staff back office, built on Tablo, Topcoat and Toasty. One example domain stands in for your product; the rest is yours to keep.
site-live-note = This site is the template as it ships. Sign up, and the app behind it is yours to try.
site-start = Start for free
site-read-source = Read the source
site-meta-locales = English and French
site-panels-title = Two panels
site-panels-note = And the way into each
site-panel-app-title = The app
site-panel-app-body = Where customers work: their teams and members, the plan they pay for, and the example projects. Anyone can sign up.
site-panel-app-sign-up = Create an account
site-panel-admin-title = Back office
site-panel-admin-body = Where staff work, apart from the app and behind a second factor. A deploy grants the role from the command line, never a page:
site-parts-title = What you keep
site-parts-note = And where it lives
site-col-part = Part
site-col-does = What it does
site-col-code = In the code
site-part-teams = Teams
site-part-teams-body = Every account gets a personal team. Owners, admins and members, invitations, ownership transfer, and a single owner, which the database enforces.
site-part-billing = Billing
site-part-billing-body = Free, Pro and Business plans defined in code and sold per seat through Stripe Checkout and the customer portal, with trials, tax, proration and a seven-day grace period on failed payments.
site-part-webhooks = Webhooks
site-part-webhooks-body = A signed inbox. Duplicate and out-of-order events refetch the state from Stripe, and a reconciliation pass runs every six hours.
site-part-jobs = Jobs
site-part-jobs-body = A durable queue in Postgres: lanes, dedupe, backoff, a dead state, schedules, and an outbox, so a job exists exactly when its write committed.
site-part-accounts = Accounts
site-part-accounts-body = Email verification, password reset, two-factor by authenticator or email, Continue with Google, account deletion, and a JSON download of everything an account owns.
site-part-backoffice = Back office
site-part-backoffice-body = A second panel, apart from the app, with two-factor required for staff: accounts, teams, subscriptions, webhook events and jobs, and an audit log of what staff did.
site-part-support = Support
site-part-support-body = Staff see a customer's pages as the customer does, for 30 minutes at most and through a second factor, without touching their credentials, membership or money.
site-part-audit = Audit
site-part-audit-body = Tablo's audit log, written in each write's transaction with the fields it changed: a team's own on the Business plan, and the back office's.
site-part-product = Your product
site-part-product-body = The example domain: a tenant-scoped resource, a form, a plan limit checked in the write's transaction, and a dashboard tile. Copy its shape, then delete it.
site-replace = Replace
site-plans-note = Read from the plan catalog
site-compare = Compare the plans
site-run-title = Run it
site-run-note = Three commands
site-run-body = You need Rust, Docker and the topcoat CLI. The app comes up at 127.0.0.1:3000, and the mail it sends is caught at 127.0.0.1:8026.
site-run-billing = With the Stripe variables empty it runs with billing off, so every team stays on the free plan.
site-run-env = the settings, from the example
site-run-services = Postgres, a Stripe mock and a mail catcher
site-run-app = the app, in development
site-pricing-title-1 = Pay per seat.
site-pricing-title-2 = Start free.
site-pricing-body = The plans live in src/billing/plans.rs. This page reads them and the Stripe catalog is created from them, so the page cannot disagree with what is charged.
site-col-plan = Plan
site-col-price = Price
site-col-seats = Seats
site-col-projects = Projects
site-col-audit = Team audit log
site-col-support = Priority support
site-included = Included
site-not-included = Not included
site-price-free = Free forever
site-unlimited = Unlimited
site-terms-body = Sample text. Replace it with terms your lawyer has approved before you launch.
site-privacy-body = Sample text. Replace it with a privacy policy that matches what you collect before you launch.
site-foot-template = Template
site-foot-built-on = Built on
site-foot-more = More
site-tablo-site = Tablo's site
site-guide = Tablo guide
site-demo = Tablo demo
site-examples = Examples gallery
site-built-with = Built with Tablo, Topcoat and Toasty.

## Labels Tablo looks up by convention

resource-project = Project
    .plural = Projects
resource-member = Member
    .plural = Members
resource-invitation = Pending invitation
    .plural = Pending invitations
field-name = Name
field-email = Email
field-password = Password
field-role = Role
field-interval = Billed
field-status = Status
field-description = Description
field-user = Person
field-actor = Who
field-at = When
field-resource = What
field-change = Change
action-manage-billing = Manage billing
action-checkout = Subscribe
action-invite = Invite someone
action-change-role = Change role
action-remove = Remove
action-transfer-ownership = Transfer ownership
action-leave = Leave team
action-revoke = Revoke
action-accept = Accept
action-decline = Decline
action-delete-team = Delete this team
action-export-account = Download my data
action-delete-account = Delete my account
hint-resync = Read this subscription back from Stripe
hint-retry = Give this job its attempts back
filter-staff = Staff
filter-customers = Customers
filter-verified = Verified
filter-unverified = Not verified
filter-processed = Processed
filter-pending = Not processed yet

field-created_at = Created
field-expires_at = Expires
field-invited_by = Invited by
field-team_name = Team
field-password_confirmation = Confirm password
field-staff_role = Staff role
field-customer_id = Stripe customer
field-plan = Plan
field-seats = Seats
field-current_period_end = Renews
field-synced_at = Last synced
field-received_at = Received
field-processed_at = Processed
field-kind = Kind
field-attempts = Attempts
field-run_at = Runs at
field-last_error = Last error
resource-account = Account
    .plural = Accounts
resource-customer = Team
    .plural = Teams
resource-subscription = Subscription
    .plural = Subscriptions
resource-webhook = Webhook
    .plural = Webhooks
resource-job = Job
    .plural = Jobs
resource-incoming-invitation = Invitation
    .plural = Invitations
option-team-role =
    .owner = Owner
    .admin = Admin
    .member = Member
option-project-status =
    .active = Active
    .paused = Paused
    .done = Done
option-staff-role =
    .administrator = Administrator
    .support = Support
option-billing-status =
    .none = No subscription
    .trialing = Trialing
    .active = Active
    .past_due = Past due
    .unpaid = Unpaid
    .canceled = Canceled
    .incomplete = Incomplete
    .incomplete_expired = Expired
    .paused = Paused
option-plan-id =
    .free = Free
    .pro = Pro
    .business = Business
option-billing-interval =
    .month = Monthly
    .year = Yearly
option-job-state =
    .queued = Queued
    .running = Running
    .succeeded = Succeeded
    .dead = Dead

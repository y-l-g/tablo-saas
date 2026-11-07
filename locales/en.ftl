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

tablo-shell-tenants = Teams
tablo-shell-switch-tenant = Switch team
tablo-shell-select-tenant = Select a team
tablo-shell-tenant-settings = Team settings
tablo-shell-create-tenant = Create team
tablo-tenant-saved = Team saved
tablo-tenant-created = Team created
tablo-tenant-create-title = Create a team
tablo-tenant-create-first = Create the team you work in on { $brand }.
tablo-tenant-create-another = Create another team to work in on { $brand }.

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
billing-manage = Manage billing
billing-off = Billing isn't configured on this server. Set the STRIPE_ variables to turn it on.
billing-per-seat = { $price } per seat per month
billing-choose-monthly = Choose monthly
billing-choose-yearly = Choose yearly, { $price } per seat
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

## Website

site-pricing = Pricing
site-sign-in = Sign in
site-sign-up = Sign up
site-terms = Terms
site-privacy = Privacy
site-headline = Run your whole team from one place.
site-lead = Everything a business needs on day one: teams, subscriptions and a back office.
site-start = Start for free
site-feature-teams = Teams with roles, invitations and an audit trail.
site-feature-billing = Subscriptions, seats and invoices through Stripe.
site-feature-security = Verified addresses and two-factor sign-in.
site-price-free = Free forever
site-unlimited = Unlimited
site-plan-seats = Seats: { $seats }
site-plan-projects = Projects: { $projects }
site-terms-body = Sample text. Replace it with terms your lawyer has approved before you launch.
site-privacy-body = Sample text. Replace it with a privacy policy that matches what you collect before you launch.
audit-deleted-actor = A deleted account

## Labels Tablo looks up by convention

resource-project = Project
    .plural = Projects
resource-member = Member
    .plural = Members
resource-invitation = Pending invitation
    .plural = Pending invitations
resource-audit = Audit entry
    .plural = Audit log
field-name = Name
field-email = Email
field-password = Password
field-role = Role
field-status = Status
field-description = Description
field-user = Person
field-actor = Who
field-at = When
field-resource = What
field-change = Change
action-invite = Invite someone
action-change-role = Change role
action-remove = Remove
action-transfer-ownership = Transfer ownership
action-leave = Leave team
action-revoke = Revoke
action-accept = Accept
action-decline = Decline
action-delete-team = Delete this team
action-delete-account = Delete my account
hint-resync = Read this subscription back from Stripe
hint-retry = Give this job its attempts back

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
resource-staff-audit = Staff audit entry
    .plural = Staff audit log
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

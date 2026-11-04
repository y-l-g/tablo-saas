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

## Navigation

navigation-team = Team

## Teams

resource-incoming-invitation = Invitation for you
    .plural = Invitations for you
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

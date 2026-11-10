# Roadmap

What this template wants from Tablo, Topcoat or Toasty and cannot get yet. Each row was checked
against the pinned revisions in `Cargo.lock`, and says what the template does in the meantime.
A row whose layer reads *Implementable now* has no upstream blocker: the
template can build it today. A row that names an upstream pull request closes once that request
merges and the template moves its pin past it. When a gap closes, delete its row along with the
workaround.

## Identity

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Social sign-in (OAuth, OpenID Connect) | Tablo | Sign-up conversion: most B2B buyers expect "Continue with Google" | Email and password, with email verification. `Auth::external` could trust an app-owned OAuth flow, but it refuses sign-up, password reset, email verification and two-factor beside it, so it replaces this panel's sign-in rather than adding to it |
| SSO (SAML, OIDC per tenant) and SCIM | Tablo | Often a condition of an enterprise contract | Not offered: `Auth::external` lets a host app sign a panel's users in, but holds for a whole panel, not per tenant, and the protocols stay the app's. `Feature` is the place to gate it once it exists. Social sign-in comes first: OIDC is its base |
| Passkeys and magic links | Tablo | Phishing-resistant sign-in, and fewer password resets | TOTP and email codes, required for staff. Magic links are the cheaper of the two, on the mailed-code machinery |

## Background work

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Jobs and schedules | A crate of its own, or Tablo | Webhooks, mail, reconciliation and pruning all need durable work | `src/jobs`: a Postgres queue with lanes, dedupe, retries, a transactional outbox and recurring schedules. An admin-panel framework may not be its home: lifting it into its own crate is as likely |
| `SELECT … FOR UPDATE SKIP LOCKED` | Toasty | The standard way to claim a job. Toasty takes locks inside its engine and exposes no lock clause on a query | A compare-and-swap on `#[version]`. Correct, but workers contend under load |

## Data

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Foreign keys, check constraints and partial unique indexes | Toasty | The database should refuse an orphan or a second owner, whatever the code does | Appended by hand to the generated migrations, between `-- #[toasty::breakpoint]` lines |
| Online migrations | Toasty | Adding an index locks a live table unless it is built concurrently | Write `CREATE INDEX CONCURRENTLY` by hand, in its own migration |
| Aggregates beyond a count | Toasty, tracked in y-l-g/tablo#118 | Revenue and usage dashboards need sums | The dashboard shows counts. MRR is left to Stripe's own reports |
| A tenant column that is a relation | Tablo | `team_id: TenantId` cannot also be a `BelongsTo`, so a row cannot name its team through a relation | Denormalized where it is read, such as `Invitation.team_name` |
| Database row-level security | Toasty, Tablo | A second wall behind `Tenancy`, in case a query forgets its scope | `Tenancy` on every resource, and `scoped_query` outside them. A test covers each cross-team path |

## Kept in the template on purpose

These are not waiting on anyone: the template owns them by design. A row moves up to a gap only if
that design changes.

| What | Why it stays here |
| --- | --- |
| Billing: plans, entitlements and a Stripe mirror (`src/billing`) | Every SaaS rebuilds them, and no crate fits yet. The module is written to be lifted into a crate of its own |
| The Stripe client (`src/billing/stripe`) | `async-stripe` was mid-rewrite at 1.0 alpha, and its generated surface is far larger than the dozen calls used here. A thin client on `reqwest`, pinned to one API version, behind the `Gateway` trait |
| Plan limits preloaded with the session | Tablo's policies are synchronous, so they cannot query. `Accounts` loads each seat's `Entitlements` with the session and a policy reads them from the context: one query per request rather than one per check |
| A staff factor older than the staff grant | Tablo requires `/app`'s factor of staff, but cannot know when a role was granted. A first factor is always proven by the password alone, so one added after the grant may be a thief's: `support::may_act_as` trusts only a factor confirmed before `staff_since`, and the Support page says so |
| Refusing what a customer owns while staff act as them | Tablo refuses the target's profile, factors and tenant settings. What else is the customer's own, their teams' members and their money, is the app's call: each such action's `can_run` refuses when `accounts::is_impersonated` |
| General rate limiting | Webhooks and public pages are flooded at the edge, before a request reaches the app: the reverse proxy limits them |

# Gaps

What this template wants from Tablo, Topcoat or Toasty and cannot get yet. Each row was checked
against the pinned revisions in `Cargo.lock`, and says what the template does in the meantime.
When a gap closes, delete its row along with the workaround.

## Identity

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Social sign-in (OAuth, OpenID Connect) | Tablo | Sign-up conversion: most B2B buyers expect "Continue with Google" | Email and password, with email verification. `Auth::external` could trust an app-owned OAuth flow, but it refuses sign-up, password reset, email verification and two-factor beside it, so it replaces this panel's sign-in rather than adding to it |
| SSO (SAML, OIDC per tenant) and SCIM | Tablo | Often a condition of an enterprise contract | Not offered: `Auth::external` lets a host app sign a panel's users in, but holds for a whole panel, not per tenant, and the protocols stay the app's. `Feature` is the place to gate it once it exists |
| Passkeys and magic links | Tablo | Phishing-resistant sign-in, and fewer password resets | TOTP and email codes, required for staff |
| Impersonation | Tablo | Support needs to see what a customer sees, with an audit trail | Not offered yet. Tablo's `Auth::impersonation` acts as another user of the same panel, but staff work in `/admin` and customers in `/app`. Offering it means a staff-only page in `/app` rendering `auth::impersonate_button` per customer, with `across_tenants` and the actor's second factor required there too |

## Billing

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Billing primitives | Tablo | Every SaaS rebuilds plans, entitlements and a Stripe mirror | `src/billing`, which is written to be lifted out |
| A Stripe client that fits | Ecosystem | `async-stripe` was mid-rewrite at 1.0 alpha, and its generated surface is far larger than the dozen calls used here | `src/billing/stripe`: a thin client on `reqwest`, pinned to one API version, behind the `Gateway` trait |
| An action's button in a page's content, its input set | Tablo | Each plan card offers a button per billing interval, which leaves for Checkout. An action renders its own button, in a page's header or a row, and asks for its input in a dialog | Checkout is a plain POST route guarded by `tablo::csrf`, rendered as a form on each card. The portal, which needs no input, is the `ManageBilling` header action, landing on Stripe with `Outcome::redirect_off_site` |
| Async policies | Tablo | Abilities are synchronous, but what a plan allows lives in a database | `Accounts` preloads each seat's `Entitlements` with the session, so a policy reads them from the context |
| A global banner slot | Tablo | "Your payment failed" belongs on every page, not on one | Shown on the billing page, plus a mail |

## Background work

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Jobs and schedules | Tablo, Topcoat | Webhooks, mail, reconciliation and pruning all need durable work | `src/jobs`: a Postgres queue with lanes, dedupe, retries, a transactional outbox and recurring schedules |
| `SELECT … FOR UPDATE SKIP LOCKED` | Toasty | The standard way to claim a job. Toasty takes locks inside its engine and exposes no lock clause on a query | A compare-and-swap on `#[version]`. Correct, but workers contend under load |
| Durable auth mail | Tablo | Verification and reset mail leaves in a `tokio::spawn` after the response. A crash or an SMTP outage loses it | `Postbox` retries three times. The person can ask for the mail again |

## Data

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Foreign keys, check constraints and partial unique indexes | Toasty | The database should refuse an orphan or a second owner, whatever the code does | Appended by hand to the generated migrations, between `-- #[toasty::breakpoint]` lines |
| Online migrations | Toasty | Adding an index locks a live table unless it is built concurrently | Write `CREATE INDEX CONCURRENTLY` by hand, in its own migration |
| Aggregates beyond a count | Toasty | Revenue and usage dashboards need sums | The dashboard shows counts. MRR is left to Stripe's own reports |
| A tenant column that is a relation | Tablo | `team_id: TenantId` cannot also be a `BelongsTo`, so a row cannot name its team through a relation | Denormalized where it is read, such as `Invitation.team_name` |
| Database row-level security | Toasty, Tablo | A second wall behind `Tenancy`, in case a query forgets its scope | `Tenancy` on every resource, and `scoped_query` outside them. A test covers each cross-team path |

## Platform

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| A shared sign-in throttle | Tablo | `LoginThrottle` counts in process memory, so N replicas allow N times the attempts, and a restart forgets them | One limit per replica. Set a stricter edge limit on `/app/login` and `/admin/login` |
| General rate limiting | Topcoat | Webhooks and public pages can be flooded | Left to the reverse proxy |
| Built-in account deletion and data export | Tablo | GDPR erasure and access requests | Deletion in `src/teams/deletion.rs`, which anonymises the audit trail. Export per resource through Tablo's CSV action; there is no whole-account export |

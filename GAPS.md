# Gaps

What this template wants from Tablo, Topcoat or Toasty and cannot get yet. Each row was checked
against the pinned revisions in `Cargo.lock`, and says what the template does in the meantime.
When a gap closes, delete its row along with the workaround.

## Identity

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Social sign-in (OAuth, OpenID Connect) | Tablo | Sign-up conversion: most B2B buyers expect "Continue with Google" | Email and password, with email verification |
| SSO (SAML, OIDC per tenant) and SCIM | Tablo | Often a condition of an enterprise contract | Not offered. `Feature` is the place to gate it once it exists |
| Passkeys and magic links | Tablo | Phishing-resistant sign-in, and fewer password resets | TOTP and email codes, required for staff |
| Impersonation | Tablo | Support needs to see what a customer sees, with an audit trail | Support reads the back office, which has no "act as" |
| One session per panel | Tablo | All panels share the `__Host-session` cookie, so signing in to `/admin` signs the same browser out of `/app`, and back. Staff who are also customers feel it daily | Use a second browser profile. Serving each panel on its own host would separate the cookies; the template has not tried it |

## Billing

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Billing primitives | Tablo | Every SaaS rebuilds plans, entitlements and a Stripe mirror | `src/billing`, which is written to be lifted out |
| A Stripe client that fits | Ecosystem | `async-stripe` was mid-rewrite at 1.0 alpha, and its generated surface is far larger than the dozen calls used here | `src/billing/stripe`: a thin client on `reqwest`, pinned to one API version, behind the `Gateway` trait |
| `Outcome::redirect` to another origin | Tablo | Checkout and the portal live on stripe.com, so they cannot be actions: `redirect` takes a same-origin path, and anything else lands back on the page after the write has committed | Plain POST routes guarded by `tablo::csrf`, rendered as forms on the billing page |
| Appending to the content security policy | Tablo | `form-action 'self'` stops a browser following a form's redirect to Stripe. Adding one origin means replacing the whole policy | `panels::content_security_policy` rewrites `DEFAULT_CONTENT_SECURITY_POLICY` by string replacement, and has to repeat what `topcoat dev` adds for the dev server |
| Async policies | Tablo | Abilities are synchronous, but what a plan allows lives in a database | `Accounts` preloads each seat's `Entitlements` with the session, so a policy reads them from the context |
| A global banner slot | Tablo | "Your payment failed" belongs on every page, not on one | Shown on the billing page, plus a mail |

## Background work

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Jobs and schedules | Tablo, Topcoat | Webhooks, mail, reconciliation and pruning all need durable work | `src/jobs`: a Postgres queue with lanes, dedupe, retries, a transactional outbox and recurring schedules |
| `SELECT … FOR UPDATE SKIP LOCKED` | Toasty | The standard way to claim a job. Toasty takes locks inside its engine and exposes no lock clause on a query | A compare-and-swap on `#[version]`. Correct, but workers contend under load |
| A recipient's locale in a job | Tablo | `PanelUser::locale` chooses a mail's language, but the language menu keeps its choice in a cookie, so an account has no stored language unless the app adds one | Job mail goes out in English, the panels' default. Auth mail, sent during the request, follows the browser |
| Durable auth mail | Tablo | Verification and reset mail leaves in a `tokio::spawn` after the response. A crash or an SMTP outage loses it | `Postbox` retries three times. The person can ask for the mail again |
| Translations outside a request | Tablo | `PanelHandle::context` promises what a job needs, but the catalog lookup is crate-private and only the router's context holds it. `PanelHandle::enter` does not help. `t!` then renders the raw id with no warning, and a mail went out with the subject `mail-invitation-subject` | `platform::words` parses the same `.ftl` files with Topcoat's `Catalog`, and fails the job on a missing id. The bidi isolates Fluent adds are stripped by hand |

## Data

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Foreign keys, check constraints and partial unique indexes | Toasty | The database should refuse an orphan or a second owner, whatever the code does | Appended by hand to the generated migrations, between `-- #[toasty::breakpoint]` lines |
| Online migrations | Toasty | Adding an index locks a live table unless it is built concurrently | Write `CREATE INDEX CONCURRENTLY` by hand, in its own migration |
| Aggregates beyond a count | Toasty | Revenue and usage dashboards need sums | The dashboard shows counts. MRR is left to Stripe's own reports |
| A tenant column that is a relation | Tablo | `team_id: TenantId` cannot also be a `BelongsTo`, so a row cannot name its team through a relation | Denormalized where it is read, such as `Invitation.team_name` |
| Database row-level security | Toasty, Tablo | A second wall behind `Tenancy`, in case a query forgets its scope | `Tenancy` on every resource, and `scoped_query` outside them. A test covers each cross-team path |

## Testing

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| Tests on the migrated schema | Tablo | `test_db` pushes the schema from the models, so hand-written foreign keys are absent. That hid a real bug: an audit row written for a team being deleted | CI applies every migration and checks them against the models. Paths through foreign keys are also run by hand |
| `TestClient::tenant` that respects membership | Tablo | It injects the tenant directly, so a cross-team test written with it proves nothing | Cross-team tests post to `/app/tenant`, the path a browser takes |
| A warning from a denied relation column | Tablo | `RelationColumn::of` on a resource the viewer cannot list renders "—" with no error, which passes every status check | `RelationColumn::unscoped`, and a test that reads the row |

## Platform

| Gap | Layer | Why a SaaS needs it | Meanwhile |
| --- | --- | --- | --- |
| A shared sign-in throttle | Tablo | `LoginThrottle` counts in process memory, so N replicas allow N times the attempts, and a restart forgets them | One limit per replica. Set a stricter edge limit on `/app/login` and `/admin/login` |
| General rate limiting | Topcoat | Webhooks and public pages can be flooded | Left to the reverse proxy |
| Built-in account deletion and data export | Tablo | GDPR erasure and access requests | Deletion in `src/teams/deletion.rs`, which anonymises the audit trail. Export per resource through Tablo's CSV action; there is no whole-account export |

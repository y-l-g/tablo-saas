# Tablo SaaS

A production SaaS template in Rust, on [Tablo](https://github.com/y-l-g/tablo),
[Topcoat](https://github.com/tokio-rs/topcoat) and [Toasty](https://github.com/tokio-rs/toasty),
with Stripe subscriptions built in. Fork it, replace the `projects` domain with your product, and
keep the rest.

## What you get

- **Teams as tenants.** Every account gets a personal team. Teams have owners, admins and members,
  invitations, ownership transfer, and a single owner, which the database enforces.
- **Per-seat subscriptions.** Free, Pro and Business plans defined in code, Stripe Checkout, the
  customer portal, trials, tax, proration, a seven-day grace period on failed payments, and plan
  limits that apply everywhere through one pure function.
- **Billing that survives Stripe.** A signed webhook inbox, duplicate and out-of-order events
  handled by refetching state from Stripe, and a reconciliation pass every six hours.
- **A durable job queue** in Postgres: lanes, dedupe, backoff, a dead state, schedules, and a
  transactional outbox, so a job exists exactly when its write committed.
- **Accounts.** Email verification, password reset, two-factor by authenticator or email, and
  account deletion that erases what it should and anonymises the rest.
- **A back office** at `/admin`, separate from the app, with two-factor required for staff: accounts,
  teams, subscriptions, webhook events and jobs, with the filters support reaches for, and an audit
  log of what staff did.
- **Audit logs** from Tablo, written in each write's transaction with the fields it changed: a
  team's own, in its settings on the Business plan, and the back office's, for administrators.
- **Public pages.** A home page, pricing rendered from the plan catalog, a sitemap and `robots.txt`.
- **Production defaults.** One image for every role, JSON logs, `/healthz` and `/readyz`, graceful
  shutdown, a configuration that refuses to start on a missing secret, English and French, with
  each account's pick kept for the mail it is sent, prices in the reader's locale, and a ribbon naming any deployment but production
  (`APP_ENVIRONMENT_NAME`).

## Run it

You need Rust, Docker and the [`topcoat` CLI](https://github.com/tokio-rs/topcoat).

```bash
cp .env.example .env
```

```bash
docker compose up -d
```

```bash
topcoat dev
```

The app is at <http://127.0.0.1:3000> and caught mail at <http://127.0.0.1:8026>. With the Stripe
variables empty it runs with billing off, so every team stays on the free plan.

To work on the back office, sign up, then give that account a role:

```bash
cargo run -- staff grant you@example.com --role administrator
```

## Connect Stripe

Use a test-mode account. In `.env`, set `STRIPE_SECRET_KEY`, then create the catalog on Stripe:

```bash
cargo run -- billing sync-catalog
```

It creates what is missing and changes nothing that exists, so it is safe on every deploy. Then
forward webhooks, and put the secret it prints in `STRIPE_WEBHOOK_SECRET`:

```bash
stripe listen --forward-to localhost:3000/webhooks/stripe
```

In production, add a webhook endpoint at `https://your-domain/webhooks/stripe` for these events:

```text
checkout.session.completed
customer.subscription.created
customer.subscription.updated
customer.subscription.deleted
customer.subscription.paused
customer.subscription.resumed
customer.subscription.trial_will_end
invoice.paid
invoice.payment_failed
```

## How it fits together

Code is grouped by what it serves, and imports go one way:

```text
cli → router → panels → domains → platform

src/
├── platform/    configuration, database, mail, telemetry, health. No business rule.
├── accounts/    who signs in
├── teams/       who works together, and deletion
├── billing/     plans, entitlements, Stripe
├── projects/    the example domain: replace it
├── audit/       what happened, per team and for staff
├── jobs/        the durable queue
├── backoffice/  what staff read
├── panels/      /app and /admin
└── site/        the public pages
```

### How a payment reaches the app

```text
Stripe ──signed POST──▶ /webhooks/stripe ──▶ stripe_events + job, in one transaction ──▶ 200
                                                             │
worker ◀─────────────────────────────────────────────────────┘
  │ refetches the subscription from Stripe
  ▼
billing_accounts ──▶ Entitlements::of(account, now) ──▶ policies, limits, the billing page
```

The rules that follow from it:

- **Stripe is the source of truth.** `billing_accounts` is a mirror. An event only says "look
  again", which is why duplicates and reordering cannot corrupt it.
- **Answer fast, work later.** The webhook stores and enqueues. A Stripe outage retries the job, and
  never fails the delivery.
- **Assume events get lost.** `ReconcileBilling` re-reads every live subscription every six hours.
- **Prices live in code.** `Plan::ALL` holds limits and amounts, and Stripe prices are found by
  lookup key. To change a price, add a key such as `pro_month_2027`. Current subscribers keep theirs.
- **One decision point.** `Entitlements::of` is pure, so "what may this team do?" has one answer and
  a unit test for each state, and a lapsed team falls to the free plan with nothing deleted.

### Tenancy

Every tenant-owned model carries `team_id`, and every resource over it declares `Tenancy`. Sessions
may only switch to a team they belong to. Code outside a resource goes through `scoped_query`. See
[`ROADMAP.md`](ROADMAP.md) for what the database does not enforce yet.

## Make it yours

1. Rename the package and brand: `Cargo.toml`, `src/panels/brand.rs`, `styles.css`, `src/site/`.
2. Set plans and limits in `src/billing/plans.rs`, then run `billing sync-catalog`.
3. Replace `src/projects/` with your product. Copy its shape: a model with `team_id`, a resource
   with `Tenancy` and a policy, and a limit from `Entitlements`.
4. Add a language by writing `locales/<code>.ftl` and listing it in `src/panels/mod.rs`. Its
   `-tenant` term names a team in every message of Tablo's.
5. Add background work with a type that implements `Job`, registered in `src/background.rs`.

## Deploy

`docker build` gives one image, and the command picks the role:

| Role | Command | Replicas |
| --- | --- | --- |
| Release step | `migrate` | Once, before the new servers start |
| Web | `serve --no-worker` | As many as the traffic needs |
| Worker | `worker` | As many as the queue needs |

A small deployment can run `serve` alone, which starts a worker beside the server. Production
refuses to start without `APP_BASE_URL` on https, `APP_COOKIE_KEY`, SMTP and the two Stripe
secrets. See [`.env.example`](.env.example) for every variable.

Before going live:

- [ ] Terminate TLS in front of the app, and set `TRUSTED_PROXIES`.
- [ ] Rate limit `/app/login` and `/admin/login` at the edge. The built-in throttle counts per
      replica.
- [ ] Keep `APP_COOKIE_KEY` stable. Rotating it locks out every enrolled authenticator.
- [ ] Back up Postgres, and test a restore.
- [ ] Point `/readyz` at your load balancer and `/healthz` at your liveness check.
- [ ] Alert on jobs in the `dead` state, visible at `/admin/jobs`.
- [ ] Run a real test-mode purchase, renewal, failed payment and cancellation. See below.

## Test it

`AGENTS.md` lists the checks CI runs. In short:

```bash
cargo test --locked
```

The suite runs on in-memory SQLite by default. Set `TABLO_TEST_DATABASE_URL` to run the integration
tests on Postgres, and `STRIPE_MOCK_URL` to check the Stripe client against
[stripe-mock](https://github.com/stripe/stripe-mock) from `compose.yaml`.

**What this does not prove.** Billing logic runs against a fake `Gateway` in tests, and the client
against stripe-mock, which validates requests but keeps no state. No test here has gone through a
live Stripe test account, so run the paid lifecycle once with your own before trusting it. Test
card `4000 0000 0000 0341` attaches and then fails, which is the quickest way to see the grace
period.

## Roadmap

[`ROADMAP.md`](ROADMAP.md) lists what Tablo, Topcoat and Toasty do not offer yet, why a SaaS
wants it, and what stands in for it here, along with what the template can build now.

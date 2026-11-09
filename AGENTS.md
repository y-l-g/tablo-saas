# Tablo SaaS — Agent Instructions

A production SaaS template built on [Tablo](https://github.com/y-l-g/tablo). When a feature needs
something Tablo, Topcoat or Toasty lacks, record it in [`GAPS.md`](GAPS.md) with what blocks it and
what the template does meanwhile.

## Layout

Code is grouped by what it serves, never by kind. Imports go one way:

```text
cli → router → panels → domains → platform
```

- `platform/` knows no business rule and imports no domain.
- `jobs/` is a domain with no business rule: other domains define jobs, and `background.rs` lists
  them all.
- A domain owns its models, resources, pages and jobs; `mod.rs` re-exports what others name.
- `Config` is the only reader of the environment.

## Rules that keep the template honest

- Every schema change is a migration: `cargo toasty migration generate --name what_changed`, and
  commit `toasty/`. Never edit an applied migration.
- Every tenant-owned model carries `team_id: TenantId`, and every resource over it declares
  `Tenancy`. Code that reads such rows goes through `scoped_query`, or states its own tenant
  predicate.
- Secrets and money never enter logs. Amounts are integer minor units.
- A foreign key, check or partial unique index is hand-appended to the generated migration, and
  tests do not see it: `test_db` pushes the schema from the models. Walk a change that touches one
  on a migrated database too.
- Stripe state is read back from Stripe, never from an event's body. Billing reaches Stripe through
  the `Gateway` trait, so tests use the fake in `tests/support/stripe.rs`.
- `Entitlements` alone decides what a plan allows. `BillingAccount.plan` is for display.
- A write and the job it implies share a transaction: pass it to `enqueue(..).exec(tx)`.
- A job writes in its reader's language: `platform::words::for_reader(cx, user.language())`, then
  `tablo::t!`. Assert on the exact text of a mail.
- A relation column on a resource the viewer cannot list renders "—". Assert on a row's text, not
  on the status, and not on the page: the sidebar shows the signed-in name.

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets --features toasty-cli --locked -- -D warnings
cargo test --locked
TABLO_TEST_DATABASE_URL=$DATABASE_URL cargo test --locked --test '*'
STRIPE_MOCK_URL=http://127.0.0.1:12111 cargo test --locked --test stripe_client
```

`docker compose up -d` starts Postgres, Mailpit and stripe-mock.

Never pipe a command whose exit code you need. Report what you ran and what you did not.

## Code and prose

- A comment explains why, never what the next line does. Every public item has a doc comment.
- Commits use Conventional Commits without a scope, e.g. `feat: bill per seat`. A PR says what
  changed and which checks ran.

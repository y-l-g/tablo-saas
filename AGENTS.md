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
- A domain owns its models, resources, pages and jobs; `mod.rs` re-exports what others name.
- `Config` is the only reader of the environment.

## Rules that keep the template honest

- Every schema change is a migration: `cargo toasty migration generate --name what_changed`, and
  commit `toasty/`. Never edit an applied migration.
- Every tenant-owned model carries `team_id: TenantId`, and every resource over it declares
  `Tenancy`. Code that reads such rows goes through `scoped_query`, or states its own tenant
  predicate.
- Secrets and money never enter logs. Amounts are integer minor units.

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets --features toasty-cli --locked -- -D warnings
cargo test --locked
TABLO_TEST_DATABASE_URL=$DATABASE_URL cargo test --locked --test '*'
```

Never pipe a command whose exit code you need. Report what you ran and what you did not.

## Code and prose

- A comment explains why, never what the next line does. Every public item has a doc comment.
- Commits use Conventional Commits without a scope, e.g. `feat: bill per seat`. A PR says what
  changed and which checks ran.

# Roadmap

| Item | Blocked by |
| --- | --- |
| Confirm a Google-only account by signing in with Google again, where a page asks for its password | Tablo |
| SSO per team (SAML, OIDC) and SCIM | Tablo |
| Passkeys and magic links | Tablo |
| Claim jobs with `SELECT … FOR UPDATE SKIP LOCKED` | Toasty |
| Declare foreign keys, checks and partial unique indexes on the models | Toasty |
| Build indexes concurrently from generated migrations | Toasty |
| Revenue and usage sums on the dashboard | Toasty, y-l-g/tablo#118 |
| Show an invitation's team name through a relation, not the `team_name` copy | Tablo: a `TenantId` column cannot be a `belongs_to` key to `Team.id`, a `Uuid`, and a `TenantId` key takes no `#[auto]` |
| Database row-level security behind `Tenancy` | Toasty, Tablo |

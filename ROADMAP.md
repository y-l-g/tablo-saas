# Roadmap

| Item | Blocked by |
| --- | --- |
| Social sign-in (OAuth, OpenID Connect) | Tablo |
| SSO per team (SAML, OIDC) and SCIM | Tablo, and social sign-in |
| Passkeys and magic links | Tablo |
| Move `src/jobs` into a crate of its own | |
| Claim jobs with `SELECT … FOR UPDATE SKIP LOCKED` | Toasty |
| Declare foreign keys, checks and partial unique indexes on the models | Toasty |
| Build indexes concurrently from generated migrations | Toasty |
| Revenue and usage sums on the dashboard | Toasty, y-l-g/tablo#118 |
| Name a row's team through a relation, not a denormalized column | Tablo |
| Database row-level security behind `Tenancy` | Toasty, Tablo |

-- Tablo's audit log replaces the team and staff trails. Its table takes the name of the team
-- trail's, so the old one steps aside until both trails are copied over.
ALTER TABLE "audit_entries" RENAME TO "team_audit_entries";
-- #[toasty::breakpoint]
ALTER INDEX "index_audit_entries_by_team_id" RENAME TO "index_team_audit_entries_by_team_id";
-- #[toasty::breakpoint]
CREATE TABLE "audit_entries" (
    "id" UUID NOT NULL,
    "panel" TEXT NOT NULL,
    "tenant" UUID,
    "actor_id" TEXT,
    "actor" TEXT NOT NULL,
    "event" TEXT NOT NULL,
    "resource" TEXT,
    "record" TEXT,
    "url" TEXT,
    "changes" TEXT NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
CREATE INDEX "index_audit_entries_by_panel_and_created_at" ON "audit_entries" ("panel", "created_at");
-- #[toasty::breakpoint]
-- An old row named its resource by a label, where the log keeps a slug, and its record by a URL.
-- The resource stays empty, so the row is listed to every reader of the trail, and the URL still
-- links. The old spellings of an event move to the log's: `action: retry` to `action:retry`.
INSERT INTO "audit_entries" ("id", "panel", "tenant", "actor_id", "actor", "event", "resource", "record", "url", "changes", "created_at")
SELECT "id", '/app', "team_id", NULL, "actor",
       REPLACE(REPLACE("change", 'action: ', 'action:'), 'force delete', 'force_delete'),
       NULL, NULL, NULLIF("record", ''), '[]', "at"
FROM "team_audit_entries";
-- #[toasty::breakpoint]
INSERT INTO "audit_entries" ("id", "panel", "tenant", "actor_id", "actor", "event", "resource", "record", "url", "changes", "created_at")
SELECT "id", '/admin', NULL, NULL, "actor",
       REPLACE(REPLACE("change", 'action: ', 'action:'), 'force delete', 'force_delete'),
       NULL, NULL, NULLIF("record", ''), '[]', "at"
FROM "staff_audit_entries";
-- #[toasty::breakpoint]
DROP TABLE "team_audit_entries";
-- #[toasty::breakpoint]
DROP TABLE "staff_audit_entries";
-- #[toasty::breakpoint]
ALTER TABLE "auth_sessions" ADD COLUMN "impersonating_until" TIMESTAMPTZ(6);
-- #[toasty::breakpoint]
ALTER TABLE "auth_sessions" ADD COLUMN "impersonating" TEXT;

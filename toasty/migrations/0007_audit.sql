CREATE TABLE "audit_entries" (
    "id" UUID NOT NULL,
    "team_id" UUID NOT NULL,
    "actor" TEXT NOT NULL,
    "resource" TEXT NOT NULL,
    "change" TEXT NOT NULL,
    "record" TEXT NOT NULL,
    "at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_audit_entries_by_team_id" ON "audit_entries" ("team_id");
CREATE TABLE "staff_audit_entries" (
    "id" UUID NOT NULL,
    "actor" TEXT NOT NULL,
    "resource" TEXT NOT NULL,
    "change" TEXT NOT NULL,
    "record" TEXT NOT NULL,
    "at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
-- #[toasty::breakpoint]
ALTER TABLE "audit_entries" ADD CONSTRAINT "audit_entries_team_id_fkey" FOREIGN KEY ("team_id") REFERENCES "teams" ("id") ON DELETE CASCADE;

CREATE TYPE "team_role" AS ENUM ('owner', 'admin', 'member');
-- #[toasty::breakpoint]
CREATE TABLE "invitations" (
    "id" UUID NOT NULL,
    "team_id" UUID NOT NULL,
    "team_name" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "role" "team_role" NOT NULL,
    "invited_by" TEXT NOT NULL,
    "expires_at" TIMESTAMPTZ(6) NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "index_invitations_by_team_id_and_email" ON "invitations" ("team_id", "email");
CREATE INDEX "index_invitations_by_team_id" ON "invitations" ("team_id");
CREATE INDEX "index_invitations_by_email" ON "invitations" ("email");
CREATE TABLE "teams" (
    "id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
CREATE TABLE "members" (
    "id" UUID NOT NULL,
    "team_id" UUID NOT NULL,
    "user_id" UUID NOT NULL,
    "role" "team_role" NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "index_members_by_team_id_and_user_id" ON "members" ("team_id", "user_id");
CREATE INDEX "index_members_by_team_id" ON "members" ("team_id");
CREATE INDEX "index_members_by_user_id" ON "members" ("user_id");
-- #[toasty::breakpoint]
-- Written by hand: Toasty declares no foreign keys or partial indexes yet. Migrations are the
-- schema's last line of defence, so what the code promises is also what the database refuses.
ALTER TABLE "members" ADD CONSTRAINT "members_team_id_fkey" FOREIGN KEY ("team_id") REFERENCES "teams" ("id") ON DELETE CASCADE;
-- #[toasty::breakpoint]
ALTER TABLE "members" ADD CONSTRAINT "members_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "users" ("id") ON DELETE CASCADE;
-- #[toasty::breakpoint]
ALTER TABLE "invitations" ADD CONSTRAINT "invitations_team_id_fkey" FOREIGN KEY ("team_id") REFERENCES "teams" ("id") ON DELETE CASCADE;
-- #[toasty::breakpoint]
CREATE UNIQUE INDEX "members_one_owner_per_team" ON "members" ("team_id") WHERE "role" = 'owner';

CREATE TYPE "plan_id" AS ENUM ('free', 'pro', 'business');
-- #[toasty::breakpoint]
CREATE TYPE "billing_status" AS ENUM ('none', 'trialing', 'active', 'past_due', 'unpaid', 'canceled', 'incomplete', 'incomplete_expired', 'paused');
-- #[toasty::breakpoint]
CREATE TYPE "billing_interval" AS ENUM ('month', 'year');
-- #[toasty::breakpoint]
CREATE TABLE "billing_accounts" (
    "id" UUID NOT NULL,
    "team_id" UUID NOT NULL,
    "customer_id" TEXT NOT NULL,
    "plan" "plan_id" NOT NULL,
    "status" "billing_status" NOT NULL,
    "subscription_id" TEXT,
    "subscription_item_id" TEXT,
    "billing_interval" "billing_interval",
    "seats" BIGINT,
    "current_period_end" TIMESTAMPTZ(6),
    "cancel_at_period_end" BOOLEAN NOT NULL,
    "trial_ends_at" TIMESTAMPTZ(6),
    "past_due_since" TIMESTAMPTZ(6),
    "synced_at" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,
    "version" BIGINT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "index_billing_accounts_by_team_id" ON "billing_accounts" ("team_id");
CREATE UNIQUE INDEX "index_billing_accounts_by_customer_id" ON "billing_accounts" ("customer_id");
-- #[toasty::breakpoint]
ALTER TABLE "billing_accounts" ADD CONSTRAINT "billing_accounts_team_id_fkey" FOREIGN KEY ("team_id") REFERENCES "teams" ("id") ON DELETE CASCADE;

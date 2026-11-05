CREATE TYPE "job_state" AS ENUM ('queued', 'running', 'succeeded', 'dead');
CREATE TABLE "job_schedules" (
    "name" TEXT NOT NULL,
    "next_run_at" TIMESTAMPTZ(6) NOT NULL,
    "version" BIGINT NOT NULL,
    PRIMARY KEY ("name")
);
CREATE TABLE "jobs" (
    "id" UUID NOT NULL,
    "kind" TEXT NOT NULL,
    "payload" TEXT NOT NULL,
    "state" "job_state" NOT NULL,
    "lane" TEXT,
    "dedupe_key" TEXT NOT NULL,
    "attempts" BIGINT NOT NULL,
    "max_attempts" BIGINT NOT NULL,
    "run_at" TIMESTAMPTZ(6) NOT NULL,
    "locked_until" TIMESTAMPTZ(6),
    "last_error" TEXT,
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    "finished_at" TIMESTAMPTZ(6),
    "version" BIGINT NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_jobs_by_kind" ON "jobs" ("kind");
CREATE INDEX "index_jobs_by_lane" ON "jobs" ("lane");
CREATE UNIQUE INDEX "index_jobs_by_dedupe_key" ON "jobs" ("dedupe_key");
CREATE INDEX "index_jobs_by_run_at" ON "jobs" ("run_at");

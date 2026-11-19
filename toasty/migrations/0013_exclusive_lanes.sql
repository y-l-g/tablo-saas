ALTER TABLE "jobs" ADD COLUMN "running_lane" TEXT;
CREATE UNIQUE INDEX "index_jobs_by_running_lane" ON "jobs" ("running_lane");

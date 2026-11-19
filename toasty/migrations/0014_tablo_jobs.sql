-- Edited by hand: the generator dropped and recreated each table's primary key as an index,
-- "index_jobs_by_id" and "index_job_schedules_by_name", which only its snapshot names. A renamed
-- table keeps its primary key, so those lines are gone.
ALTER TABLE "jobs" RENAME TO "tablo_jobs";
DROP INDEX "index_jobs_by_kind";
DROP INDEX "index_jobs_by_lane";
DROP INDEX "index_jobs_by_running_lane";
DROP INDEX "index_jobs_by_dedupe_key";
DROP INDEX "index_jobs_by_run_at";
CREATE INDEX "index_tablo_jobs_by_kind" ON "tablo_jobs" ("kind");
CREATE UNIQUE INDEX "index_tablo_jobs_by_dedupe_key" ON "tablo_jobs" ("dedupe_key");
CREATE INDEX "index_tablo_jobs_by_run_at" ON "tablo_jobs" ("run_at");
CREATE INDEX "index_tablo_jobs_by_lane" ON "tablo_jobs" ("lane");
CREATE UNIQUE INDEX "index_tablo_jobs_by_running_lane" ON "tablo_jobs" ("running_lane");
ALTER TABLE "job_schedules" RENAME TO "tablo_job_schedules";
ALTER TABLE "tablo_job_schedules" RENAME COLUMN "name" TO "kind";
-- #[toasty::breakpoint]
-- Written by hand: `tablo::jobs` seals auth mail under a key of its own and prunes the queue
-- itself, so no worker runs the app's `send-mail` and `prune-jobs` any more. A letter still
-- queued is lost, and its person asks for it again.
DELETE FROM "tablo_jobs" WHERE "kind" IN ('send-mail', 'prune-jobs');
DELETE FROM "tablo_job_schedules" WHERE "kind" = 'prune-jobs';

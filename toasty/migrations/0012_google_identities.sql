CREATE TABLE "identities" (
    "id" UUID NOT NULL,
    "user_id" UUID NOT NULL,
    "issuer" TEXT NOT NULL,
    "subject" TEXT NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "index_identities_by_issuer_and_subject" ON "identities" ("issuer", "subject");
CREATE INDEX "index_identities_by_user_id" ON "identities" ("user_id");
-- #[toasty::breakpoint]
-- Written by hand: Toasty declares no foreign keys yet.
ALTER TABLE "identities" ADD CONSTRAINT "identities_user_id_fkey" FOREIGN KEY ("user_id") REFERENCES "users" ("id") ON DELETE CASCADE;

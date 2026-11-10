CREATE TABLE "auth_throttles" (
    "key" TEXT NOT NULL,
    "scope" TEXT NOT NULL,
    "attempts" BIGINT NOT NULL,
    "since" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("key")
);
CREATE INDEX "index_auth_throttles_by_scope" ON "auth_throttles" ("scope");

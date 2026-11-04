CREATE TABLE "column_preferences" (
    "key" TEXT NOT NULL,
    "user_id" TEXT NOT NULL,
    "panel" TEXT NOT NULL,
    "resource" TEXT NOT NULL,
    "columns" TEXT NOT NULL,
    PRIMARY KEY ("key")
);
CREATE INDEX "index_column_preferences_by_user_id" ON "column_preferences" ("user_id");
CREATE TABLE "auth_factors" (
    "id" UUID NOT NULL,
    "user_id" TEXT NOT NULL,
    "panel" TEXT NOT NULL,
    "kind" TEXT NOT NULL,
    "secret" TEXT NOT NULL,
    "confirmed_at" TIMESTAMPTZ(6),
    "last_step" BIGINT,
    "failures" BIGINT NOT NULL,
    "retry_at" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_auth_factors_by_user_id" ON "auth_factors" ("user_id");
CREATE TABLE "stored_notifications" (
    "id" UUID NOT NULL,
    "user_id" TEXT NOT NULL,
    "panel" TEXT NOT NULL,
    "tenant" UUID,
    "notification" TEXT NOT NULL,
    "read_at" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
CREATE INDEX "index_stored_notifications_by_user_id_and_panel_and_created_at" ON "stored_notifications" ("user_id", "panel", "created_at");
CREATE TABLE "admin_users" (
    "id" UUID NOT NULL,
    "email" TEXT NOT NULL,
    "password_hash" TEXT NOT NULL,
    "display_name" TEXT NOT NULL,
    "active" BOOLEAN NOT NULL,
    "email_verified_at" TIMESTAMPTZ(6),
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "index_admin_users_by_email" ON "admin_users" ("email");
CREATE TABLE "auth_tokens" (
    "token_hash" TEXT NOT NULL,
    "purpose" TEXT NOT NULL,
    "user_id" TEXT NOT NULL,
    "panel" TEXT NOT NULL,
    "email" TEXT,
    "expires_at" TIMESTAMPTZ(6) NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("token_hash")
);
CREATE INDEX "index_auth_tokens_by_user_id" ON "auth_tokens" ("user_id");
CREATE INDEX "index_auth_tokens_by_expires_at" ON "auth_tokens" ("expires_at");
CREATE TABLE "auth_sessions" (
    "token_hash" TEXT NOT NULL,
    "user_id" TEXT NOT NULL,
    "panel" TEXT NOT NULL,
    "tenant" UUID,
    "expires_at" TIMESTAMPTZ(6) NOT NULL,
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("token_hash")
);
CREATE INDEX "index_auth_sessions_by_user_id" ON "auth_sessions" ("user_id");
CREATE INDEX "index_auth_sessions_by_expires_at" ON "auth_sessions" ("expires_at");

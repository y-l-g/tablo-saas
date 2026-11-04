CREATE TYPE "staff_role" AS ENUM ('administrator', 'support');
CREATE TABLE "users" (
    "id" UUID NOT NULL,
    "name" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "email_verified_at" TIMESTAMPTZ(6),
    "password" TEXT NOT NULL,
    "staff_role" "staff_role",
    "created_at" TIMESTAMPTZ(6) NOT NULL,
    "updated_at" TIMESTAMPTZ(6) NOT NULL,
    PRIMARY KEY ("id")
);
CREATE UNIQUE INDEX "index_users_by_email" ON "users" ("email");

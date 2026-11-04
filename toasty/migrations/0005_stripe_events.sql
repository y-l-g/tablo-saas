CREATE TABLE "stripe_events" (
    "id" TEXT NOT NULL,
    "kind" TEXT NOT NULL,
    "customer_id" TEXT NOT NULL,
    "received_at" TIMESTAMPTZ(6) NOT NULL,
    "processed_at" TIMESTAMPTZ(6),
    PRIMARY KEY ("id")
);
CREATE INDEX "index_stripe_events_by_customer_id" ON "stripe_events" ("customer_id");

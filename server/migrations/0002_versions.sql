-- Every editable entity carries a version, bumped on each edit and exposed as
-- the API's ETag; edits must send it back in If-Match (design §5).
ALTER TABLE commodity    ADD COLUMN version bigint NOT NULL DEFAULT 1;
ALTER TABLE party        ADD COLUMN version bigint NOT NULL DEFAULT 1;
ALTER TABLE account      ADD COLUMN version bigint NOT NULL DEFAULT 1;
ALTER TABLE counterparty ADD COLUMN version bigint NOT NULL DEFAULT 1;
ALTER TABLE category     ADD COLUMN version bigint NOT NULL DEFAULT 1;
ALTER TABLE tag          ADD COLUMN version bigint NOT NULL DEFAULT 1;

-- A replayed 204 has no body.
ALTER TABLE idempotency_key ALTER COLUMN response_body DROP NOT NULL;
-- Old keys are pruned when new ones are stored (there is no cron).
CREATE INDEX idempotency_key_created ON idempotency_key (created_at);

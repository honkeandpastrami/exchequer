-- Every transaction belongs to the import batch it entered with (design §6.7):
-- a migration, an uploaded file, or a transaction entered by hand (a batch of
-- its own). Edits never change it.
CREATE TABLE import_batch (
    id         uuid PRIMARY KEY,
    kind       text NOT NULL CHECK (kind IN ('manual', 'migration', 'file')),
    label      text CHECK (label <> '' AND length(label) <= 200),
    token_id   uuid REFERENCES api_token,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- Bumped whenever a transaction in the batch is added, changed or
    -- removed; the API's ETag, so undoing a batch acts on what was seen.
    version    bigint NOT NULL DEFAULT 1
);

ALTER TABLE transaction ADD COLUMN import_batch_id uuid REFERENCES import_batch;

-- Backfill from the audit log: who created each transaction, and when.
CREATE TEMPORARY TABLE created_by ON COMMIT DROP AS
    SELECT DISTINCT ON (entity_id) entity_id AS transaction_id, token_id, at
    FROM audit_log
    WHERE entity = 'transaction' AND action = 'create'
    ORDER BY entity_id, at;

-- A migration writes into an empty server with its own token: everything a
-- token created is one migration batch if any of it came from outside.
CREATE TEMPORARY TABLE migrated ON COMMIT DROP AS
    SELECT c.token_id, gen_random_uuid() AS batch_id, min(c.at) AS at,
           min(e.source) AS source
    FROM created_by c
    JOIN transaction_external_ref e USING (transaction_id)
    GROUP BY c.token_id;

INSERT INTO import_batch (id, kind, label, token_id, created_at)
    SELECT batch_id, 'migration', 'Migrated from ' || source, token_id, at FROM migrated;

UPDATE transaction t SET import_batch_id = m.batch_id
    FROM created_by c JOIN migrated m ON m.token_id IS NOT DISTINCT FROM c.token_id
    WHERE c.transaction_id = t.id;

-- Everything else was entered by hand: one batch each, with the
-- transaction's own ID.
INSERT INTO import_batch (id, kind, token_id, created_at)
    SELECT t.id, 'manual', c.token_id, t.created_at
    FROM transaction t LEFT JOIN created_by c ON c.transaction_id = t.id
    WHERE t.import_batch_id IS NULL;
UPDATE transaction SET import_batch_id = id WHERE import_batch_id IS NULL;

ALTER TABLE transaction ALTER COLUMN import_batch_id SET NOT NULL;
CREATE INDEX transaction_import_batch ON transaction (import_batch_id);

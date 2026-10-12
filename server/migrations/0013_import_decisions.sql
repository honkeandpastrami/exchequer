-- What the client decided for each staged row (design §6.6-§6.7), and the
-- labels it gave the transaction the row will become. A null decision means
-- the default for the row's status.
ALTER TABLE staged_row
    ADD COLUMN decision        text CHECK (decision IN ('match', 'new', 'skip')),
    ADD COLUMN set_description text CHECK (set_description <> '' AND length(set_description) <= 500),
    ADD COLUMN counterparty_id uuid REFERENCES counterparty ON DELETE SET NULL,
    ADD COLUMN category_id     uuid REFERENCES category ON DELETE SET NULL,
    ADD COLUMN purpose_id      uuid REFERENCES party ON DELETE SET NULL;

-- The file's identity: its bytes' SHA-256 (hex), whatever it's called.
ALTER TABLE staged_import ADD COLUMN file_sha256 text NOT NULL DEFAULT ''
    CHECK (file_sha256 ~ '^([0-9a-f]{64})?$');
ALTER TABLE staged_import ALTER COLUMN file_sha256 DROP DEFAULT;

-- Each bank row as each file had it: what a row adds to the transaction it
-- became or matched (design §6.7), since files aren't kept. One per file,
-- however many copies of it are imported.
CREATE TABLE imported_row (
    account_id  uuid NOT NULL,
    fingerprint text NOT NULL,
    file_sha256 text NOT NULL CHECK (file_sha256 ~ '^[0-9a-f]{64}$'),
    raw         jsonb NOT NULL,
    imported_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, fingerprint, file_sha256),
    FOREIGN KEY (account_id, fingerprint) REFERENCES import_fingerprint ON DELETE CASCADE
);

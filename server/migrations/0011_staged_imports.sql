-- Uploaded bank files, staged for review before they become transactions
-- (design §6). The file itself isn't kept: only its statements and each
-- row's parsed and raw forms, until the import is committed or discarded.
CREATE TABLE staged_import (
    id         uuid PRIMARY KEY,
    file_name  text CHECK (file_name <> '' AND length(file_name) <= 200),
    format     text NOT NULL CHECK (format IN ('ofx', 'csv')),
    token_id   uuid REFERENCES api_token,
    created_at timestamptz NOT NULL DEFAULT now(),
    -- Bumped on every change; the API's ETag.
    version    bigint NOT NULL DEFAULT 1
);

-- One account's part of a file. The account is resolved from the number in
-- the file, or given by the client; until then its rows need an account.
CREATE TABLE staged_statement (
    import_id      uuid NOT NULL REFERENCES staged_import ON DELETE CASCADE,
    position       smallint NOT NULL CHECK (position >= 0),
    -- The number as the file wrote it (normalized), so it can be added to
    -- the account the client picks. Only ever shown by its last four.
    account_number text CHECK (account_number <> ''),
    account_id     uuid REFERENCES account ON DELETE SET NULL,
    kind           text CHECK (kind IN ('bank', 'credit_card')),
    commodity_id   uuid NOT NULL REFERENCES commodity,
    ledger_amount  numeric,
    ledger_date    date,
    -- Lines the parser couldn't read: [{"line": n, "reason": "..."}].
    skipped        jsonb NOT NULL DEFAULT '[]',
    PRIMARY KEY (import_id, position),
    CHECK ((ledger_amount IS NULL) = (ledger_date IS NULL))
);

CREATE TABLE staged_row (
    import_id   uuid NOT NULL,
    statement   smallint NOT NULL,
    position    integer NOT NULL CHECK (position >= 0),
    date        date NOT NULL,
    -- Signed from the account's side: negative leaves it.
    amount      numeric NOT NULL CHECK (amount <> 0),
    description text NOT NULL,
    native_id   text,
    -- The row's ID for exact duplicates (see import_fingerprint).
    fingerprint text NOT NULL CHECK (fingerprint <> ''),
    raw         jsonb NOT NULL,
    status      text NOT NULL CHECK (status IN
                    ('new', 'duplicate', 'near_duplicate', 'transfer_match', 'needs_account')),
    -- The existing transaction a duplicate or match is of.
    match_transaction_id uuid REFERENCES transaction ON DELETE SET NULL,
    PRIMARY KEY (import_id, statement, position),
    FOREIGN KEY (import_id, statement) REFERENCES staged_statement ON DELETE CASCADE
);

-- Each imported row's ID, per account: the same row never enters twice
-- (design §6.3). Goes with its transaction.
CREATE TABLE import_fingerprint (
    account_id     uuid NOT NULL REFERENCES account ON DELETE CASCADE,
    fingerprint    text NOT NULL CHECK (fingerprint <> '' AND length(fingerprint) <= 200),
    transaction_id uuid NOT NULL REFERENCES transaction ON DELETE CASCADE,
    PRIMARY KEY (account_id, fingerprint)
);
CREATE INDEX ON import_fingerprint (transaction_id);

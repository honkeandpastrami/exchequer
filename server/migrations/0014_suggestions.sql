-- Suggestions in staging (design §8, docs/ai.md): a statement's account and
-- each row's counterparty, category and purpose may be suggested by rules,
-- local AI or remote AI, each with a confidence. Review shows them; commit
-- takes the confident ones unless you chose otherwise.

-- The exact system that made a suggestion: kind, name and version, plus what
-- else identifies it (a model's version as the service reports it, the data a
-- local model learned from, the version of the questions asked).
CREATE TABLE suggester (
    id         uuid PRIMARY KEY,
    kind       text NOT NULL CHECK (kind IN ('rule', 'local', 'remote')),
    name       text NOT NULL CHECK (name <> '' AND length(name) <= 200),
    version    text NOT NULL CHECK (version <> '' AND length(version) <= 200),
    details    jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(details) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (kind, name, version, details)
);

-- A statement's suggested account, while it has none of its own (from the
-- file's number or your choice). A confident one (confidence >= 900) counts
-- as the account until you pick another.
ALTER TABLE staged_statement
    ADD COLUMN suggested_account_id uuid REFERENCES account ON DELETE SET NULL,
    ADD COLUMN account_suggester_id uuid REFERENCES suggester,
    ADD COLUMN account_confidence   smallint CHECK (account_confidence BETWEEN 0 AND 1000),
    ADD COLUMN account_reason       text CHECK (account_reason <> '' AND length(account_reason) <= 500),
    ADD CONSTRAINT staged_statement_suggestion CHECK (
        (suggested_account_id IS NULL) = (account_suggester_id IS NULL)
        AND (suggested_account_id IS NULL) = (account_confidence IS NULL)
        AND (suggested_account_id IS NULL) = (account_reason IS NULL));

-- Which of a row's labels you set yourself; a null label you set is an
-- explicit "none" (unknown counterparty, no category, the account's owner).
-- Labels set before this migration were set by you.
ALTER TABLE staged_row ADD COLUMN chosen text[] NOT NULL DEFAULT '{}'
    CHECK (chosen <@ ARRAY['counterparty', 'category', 'purpose']);
UPDATE staged_row SET chosen = array_remove(ARRAY[
    CASE WHEN counterparty_id IS NOT NULL THEN 'counterparty' END,
    CASE WHEN category_id IS NOT NULL THEN 'category' END,
    CASE WHEN purpose_id IS NOT NULL THEN 'purpose' END], NULL);

-- A suggested label for a staged row: at most one per label, the best any
-- level made. A null value suggests "none". The suggestion goes if what it
-- names is deleted.
CREATE TABLE staged_suggestion (
    import_id       uuid NOT NULL,
    statement       smallint NOT NULL,
    position        integer NOT NULL,
    label           text NOT NULL CHECK (label IN ('counterparty', 'category', 'purpose')),
    counterparty_id uuid REFERENCES counterparty ON DELETE CASCADE,
    category_id     uuid REFERENCES category ON DELETE CASCADE,
    purpose_id      uuid REFERENCES party ON DELETE CASCADE,
    suggester_id    uuid NOT NULL REFERENCES suggester,
    -- Per mille: 900 means 0.9.
    confidence      smallint NOT NULL CHECK (confidence BETWEEN 0 AND 1000),
    reason          text NOT NULL CHECK (reason <> '' AND length(reason) <= 500),
    PRIMARY KEY (import_id, statement, position, label),
    FOREIGN KEY (import_id, statement, position) REFERENCES staged_row ON DELETE CASCADE,
    CHECK (label = 'counterparty' OR counterparty_id IS NULL),
    CHECK (label = 'category' OR category_id IS NULL),
    CHECK (label = 'purpose' OR purpose_id IS NULL)
);
CREATE INDEX ON staged_suggestion (suggester_id);

-- What each suggestion became on commit, to measure each suggester: what it
-- suggested, how sure it was, and what was committed. A log, not part of the
-- ledger: it outlives the transactions it describes.
CREATE TABLE suggestion_outcome (
    id             uuid PRIMARY KEY,
    suggester_id   uuid NOT NULL REFERENCES suggester,
    label          text NOT NULL
                   CHECK (label IN ('account', 'counterparty', 'category', 'purpose')),
    suggested      uuid,
    confidence     smallint NOT NULL CHECK (confidence BETWEEN 0 AND 1000),
    committed      uuid,
    -- The transaction the row became; none for an account.
    transaction_id uuid,
    committed_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX ON suggestion_outcome (suggester_id, label);

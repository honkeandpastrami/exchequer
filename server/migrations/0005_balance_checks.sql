-- Balance checks (design §7.1): what the bank showed for an account on a
-- date. Whether a check matches is computed against the ledger every time,
-- never stored, so later edits to the ledger are reflected.

CREATE TABLE balance_check (
    id           uuid PRIMARY KEY,
    account_id   uuid NOT NULL REFERENCES account ON DELETE CASCADE,
    commodity_id uuid NOT NULL REFERENCES commodity,
    date         date NOT NULL,
    -- In the ledger's sign (+ held, − owed); the API converts for accounts
    -- whose banks show amounts owed as positive.
    balance      numeric NOT NULL,
    source       text NOT NULL CHECK (source IN ('user', 'ofx')),
    created_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX balance_check_account ON balance_check (account_id, commodity_id, date);

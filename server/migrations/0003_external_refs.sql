-- Where a transaction came from outside Exchequer (design §3.1 provenance),
-- e.g. its ID in the Firefly III instance it was migrated from (§11). Unique
-- per source, so the same outside record can't be stored twice.
CREATE TABLE transaction_external_ref (
    transaction_id uuid NOT NULL REFERENCES transaction ON DELETE CASCADE,
    source         text NOT NULL CHECK (source ~ '^[a-z0-9_-]{1,32}$'),
    ref            text NOT NULL CHECK (ref <> '' AND length(ref) <= 200),
    PRIMARY KEY (source, ref)
);
CREATE INDEX transaction_external_ref_transaction ON transaction_external_ref (transaction_id);

-- Archived accounts are kept for history but hidden from everyday views.
ALTER TABLE account ADD COLUMN archived boolean NOT NULL DEFAULT false;

-- A credit limit is display metadata (design §3.1), in one commodity.
ALTER TABLE account ADD COLUMN credit_limit_commodity_id uuid REFERENCES commodity;
ALTER TABLE account ADD COLUMN credit_limit numeric CHECK (credit_limit > 0);
ALTER TABLE account ADD CONSTRAINT account_credit_limit_complete
    CHECK ((credit_limit IS NULL) = (credit_limit_commodity_id IS NULL));

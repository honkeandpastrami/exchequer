-- What the client decided for each staged row (design §6.6-§6.7), and the
-- labels it gave the transaction the row will become. A null decision means
-- the default for the row's status.
ALTER TABLE staged_row
    ADD COLUMN decision        text CHECK (decision IN ('match', 'new', 'skip')),
    ADD COLUMN set_description text CHECK (set_description <> '' AND length(set_description) <= 500),
    ADD COLUMN counterparty_id uuid REFERENCES counterparty ON DELETE SET NULL,
    ADD COLUMN category_id     uuid REFERENCES category ON DELETE SET NULL,
    ADD COLUMN purpose_id      uuid REFERENCES party ON DELETE SET NULL;

-- Each imported row's fields as the bank file had them: the transaction's
-- provenance, since the file itself isn't kept (§6.7). None for rows recorded
-- some other way (e.g. a migration).
ALTER TABLE import_fingerprint ADD COLUMN raw jsonb;

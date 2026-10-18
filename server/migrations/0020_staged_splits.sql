-- A staged row split into parts before commit (design §6.7): each part is a
-- split of the new transaction, between the statement's account and its own
-- other side. The parts add up to the row's amount; all their labels are
-- the client's, so a split row takes no suggestions.
CREATE TABLE staged_split (
    import_id   uuid NOT NULL,
    statement   smallint NOT NULL,
    position    integer NOT NULL,
    part        smallint NOT NULL CHECK (part >= 0),
    -- Unsigned: the row's sign says which way the money goes.
    amount      numeric NOT NULL CHECK (amount > 0),
    counterparty_id     uuid REFERENCES counterparty ON DELETE SET NULL,
    transfer_account_id uuid REFERENCES account ON DELETE SET NULL,
    category_id uuid REFERENCES category ON DELETE SET NULL,
    purpose_id  uuid REFERENCES party ON DELETE SET NULL,
    description text CHECK (description <> '' AND length(description) <= 500),
    PRIMARY KEY (import_id, statement, position, part),
    FOREIGN KEY (import_id, statement, position) REFERENCES staged_row ON DELETE CASCADE,
    CHECK (counterparty_id IS NULL OR transfer_account_id IS NULL)
);

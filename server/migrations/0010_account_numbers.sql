-- Account numbers a bank file may name an account by (design §6.2). An
-- account can have several, e.g. a card whose payments and purchases arrive
-- under different numbers; a number belongs to one account only. Stored
-- normalized (no spaces or dashes), and only ever shown as its last four.
CREATE TABLE account_number (
    id         uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES account ON DELETE CASCADE,
    number     text NOT NULL UNIQUE CHECK (number ~ '^[0-9A-Za-z]{1,34}$')
);
CREATE INDEX ON account_number (account_id);

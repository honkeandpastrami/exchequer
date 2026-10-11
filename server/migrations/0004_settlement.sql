-- Household settlement (design §4.3, §4.4).
--
-- Statements are never stored: they're computed from the ledger and these
-- parameters every time (§4.4). What is stored is each month's stipends and
-- funders, whether the month is closed, and the months settlement payments
-- settle when that isn't the default.

-- Settlement values are in the household currency, set on the household.
ALTER TABLE party ADD COLUMN currency_id uuid REFERENCES commodity;
ALTER TABLE party ADD CONSTRAINT party_currency_household
    CHECK (currency_id IS NULL OR kind = 'household');

-- The month a settlement payment settles, when not the one before it's made.
-- Stored as the month's first day.
ALTER TABLE split ADD COLUMN settles date CHECK (extract(day FROM settles) = 1);

-- A row exists once a month's parameters are set.
CREATE TABLE settlement (
    month     date PRIMARY KEY CHECK (extract(day FROM month) = 1),
    -- Names the month in the audit log.
    id        uuid NOT NULL UNIQUE,
    closed_at timestamptz,
    version   bigint NOT NULL DEFAULT 1
);

-- What the Household owes each person for the month (people only; checked by
-- the API).
CREATE TABLE settlement_stipend (
    month    date NOT NULL REFERENCES settlement ON DELETE CASCADE,
    party_id uuid NOT NULL REFERENCES party,
    amount   numeric NOT NULL CHECK (amount >= 0),
    PRIMARY KEY (month, party_id)
);

-- Each business's funder for the month: the household or a person.
CREATE TABLE settlement_funder (
    month       date NOT NULL REFERENCES settlement ON DELETE CASCADE,
    business_id uuid NOT NULL REFERENCES party,
    funder_id   uuid NOT NULL REFERENCES party,
    PRIMARY KEY (month, business_id),
    CHECK (business_id <> funder_id)
);

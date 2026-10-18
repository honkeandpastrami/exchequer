-- A mortgage is never liquid: money paid into it is gone for a long time,
-- so it's spending, not a transfer (design §4.1).
UPDATE account SET liquid = false WHERE kind = 'mortgage';
ALTER TABLE account ADD CONSTRAINT account_mortgage_not_liquid
    CHECK (kind <> 'mortgage' OR NOT liquid);

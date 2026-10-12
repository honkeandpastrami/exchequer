-- Settlement is always in USD (design §4.3): the household currency setting
-- added in 0004 goes away.

ALTER TABLE party DROP COLUMN currency_id;

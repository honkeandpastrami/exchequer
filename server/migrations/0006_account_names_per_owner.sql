-- Account names are unique per owner, not across the household: two people
-- can each have a "Checking". Anywhere an account is shown on its own,
-- it's shown with its owner.

ALTER TABLE account DROP CONSTRAINT account_name_key;
ALTER TABLE account ADD CONSTRAINT account_owner_name_key UNIQUE (owner_id, name);

-- A leg outside the household may leave its counterparty unknown (no
-- account and no counterparty), instead of a placeholder counterparty.
ALTER TABLE split DROP CONSTRAINT split_check;
ALTER TABLE split DROP CONSTRAINT split_check1;
ALTER TABLE split ADD CONSTRAINT split_from_one_side
    CHECK (num_nonnulls(from_account_id, from_counterparty_id) <= 1);
ALTER TABLE split ADD CONSTRAINT split_to_one_side
    CHECK (num_nonnulls(to_account_id, to_counterparty_id) <= 1);

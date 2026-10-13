-- A duplicate or near-duplicate row may replace the transaction it matches:
-- that transaction takes the bank's date, amount and description (design
-- §6.7).
ALTER TABLE staged_row DROP CONSTRAINT staged_row_decision_check;
ALTER TABLE staged_row ADD CONSTRAINT staged_row_decision_check
    CHECK (decision IN ('match', 'new', 'skip', 'replace'));

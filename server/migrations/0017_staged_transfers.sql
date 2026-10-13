-- Transfers in staging (design §6.4): a row's other side may be one of your
-- accounts, and two staged rows (in any staged imports) may be the two sides
-- of one transfer, committed as one. Matching staged rows to one-sided
-- transactions in the ledger goes: a transfer is recorded as one when its
-- first side is committed.

-- The other side as one of your accounts, when you chose it: a transfer. A
-- row's other side is a counterparty or an account, never both.
ALTER TABLE staged_row
    ADD COLUMN transfer_account_id uuid REFERENCES account ON DELETE SET NULL,
    ADD CONSTRAINT staged_row_other_side
        CHECK (counterparty_id IS NULL OR transfer_account_id IS NULL);

-- A transfer match: the staged row that is the other side of the same
-- transfer. Goes when that row does.
ALTER TABLE staged_row
    ADD COLUMN pair_import_id uuid,
    ADD COLUMN pair_statement smallint,
    ADD COLUMN pair_position  integer,
    ADD CONSTRAINT staged_row_pair
        FOREIGN KEY (pair_import_id, pair_statement, pair_position)
        REFERENCES staged_row ON DELETE SET NULL,
    ADD CONSTRAINT staged_row_pair_whole
        CHECK ((pair_import_id IS NULL) = (pair_statement IS NULL)
               AND (pair_import_id IS NULL) = (pair_position IS NULL));

-- `transfer_match` now means a staged pair; a ledger transfer already
-- holding the row's other side is a `transfer_duplicate`. Rows matched the
-- old way go back to new.
UPDATE staged_row SET decision = NULL WHERE status = 'transfer_match' AND decision = 'match';
UPDATE staged_row SET status = 'new', match_transaction_id = NULL
WHERE status = 'transfer_match';
ALTER TABLE staged_row DROP CONSTRAINT staged_row_status_check;
ALTER TABLE staged_row ADD CONSTRAINT staged_row_status_check CHECK (status IN
    ('new', 'duplicate', 'near_duplicate', 'transfer_match', 'transfer_duplicate',
     'needs_account'));

-- A suggested other side may be one of your accounts.
ALTER TABLE staged_suggestion
    ADD COLUMN account_id uuid REFERENCES account ON DELETE CASCADE,
    ADD CONSTRAINT staged_suggestion_account
        CHECK (account_id IS NULL OR (label = 'counterparty' AND counterparty_id IS NULL));

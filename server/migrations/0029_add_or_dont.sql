-- Staging as add or don't add (design §6.6-§6.7): each row has a label
-- (what matching found) and a choice with a default; your choices win, and
-- a row's colour says what it needs.

-- Labels in the words review shows. A recorded transfer is a duplicate (the
-- same day) or a possible duplicate; how a match was found is its kind.
ALTER TABLE staged_row
    ADD COLUMN match_kind text CHECK (match_kind IN ('bank_id', 'transfer', 'amount'));
UPDATE staged_row SET match_kind = 'bank_id' WHERE status = 'duplicate';
UPDATE staged_row SET match_kind = 'amount' WHERE status = 'near_duplicate';
UPDATE staged_row SET match_kind = 'transfer' WHERE status = 'transfer_duplicate';
-- A transfer conflict is a transfer pair whose default is to add the two
-- rows separately: your choice of the other side stays the default.
UPDATE staged_row SET status = 'transfer_match', decision = NULL
WHERE conflict_import_id IS NOT NULL AND status = 'new';

ALTER TABLE staged_row DROP CONSTRAINT staged_row_status_check;
ALTER TABLE staged_row DROP CONSTRAINT staged_row_decision_check;
-- Choices in the same words: add (one transfer, for a pair), add
-- separately (a pair's two rows, each new) or don't add (a duplicate's
-- links it to what it matched). Replace goes.
UPDATE staged_row SET decision = CASE
    WHEN decision = 'replace' THEN NULL
    WHEN status IN ('duplicate', 'near_duplicate', 'transfer_duplicate') THEN
        CASE decision WHEN 'match' THEN 'dont_add' WHEN 'new' THEN 'add' END
    WHEN status = 'transfer_match' THEN
        CASE decision WHEN 'match' THEN 'add' WHEN 'new' THEN 'add_separately'
                      WHEN 'skip' THEN 'dont_add' END
    ELSE CASE decision WHEN 'new' THEN 'add' WHEN 'skip' THEN 'dont_add' END
END
WHERE decision IS NOT NULL;
UPDATE staged_row SET status = CASE status
    WHEN 'near_duplicate' THEN 'possible_duplicate'
    WHEN 'transfer_duplicate' THEN
        CASE WHEN match_days = 0 THEN 'duplicate' ELSE 'possible_duplicate' END
    WHEN 'transfer_match' THEN 'transfer_pair'
    WHEN 'needs_account' THEN 'no_account'
    ELSE status
END;
ALTER TABLE staged_row ADD CONSTRAINT staged_row_status_check CHECK (status IN
    ('new', 'duplicate', 'possible_duplicate', 'transfer_pair', 'no_account'));
ALTER TABLE staged_row ADD CONSTRAINT staged_row_decision_check
    CHECK (decision IN ('add', 'add_separately', 'dont_add'));
ALTER TABLE staged_row ADD CONSTRAINT staged_row_match_kind
    CHECK ((match_kind IS NULL) = (status NOT IN ('duplicate', 'possible_duplicate')));

-- What a suggested decision became, in either set of words.
ALTER TABLE suggestion_outcome DROP CONSTRAINT suggestion_outcome_suggested_decision_check;
ALTER TABLE suggestion_outcome DROP CONSTRAINT suggestion_outcome_committed_decision_check;
ALTER TABLE suggestion_outcome ADD CONSTRAINT suggestion_outcome_suggested_decision_check
    CHECK (suggested_decision IN
        ('match', 'new', 'skip', 'replace', 'add', 'add_separately', 'dont_add'));
ALTER TABLE suggestion_outcome ADD CONSTRAINT suggestion_outcome_committed_decision_check
    CHECK (committed_decision IN
        ('match', 'new', 'skip', 'replace', 'add', 'add_separately', 'dont_add'));

-- A statement's suggested account you ticked as reviewed: it's the
-- statement's own now (`account_id`), and this says it came from the tick.
ALTER TABLE staged_statement ADD COLUMN reviewed boolean NOT NULL DEFAULT false;

-- Tags for the new transaction, by name (created on commit if new).
ALTER TABLE staged_row ADD COLUMN tags text[] NOT NULL DEFAULT '{}';

-- A row's decision is a suggestion too (design §6.7, docs/ai.md): matching
-- suggests what committing does with each row, as sure as what it found.
-- Its suggester is of a kind of its own: logic, like the rules.
ALTER TABLE suggester DROP CONSTRAINT suggester_kind_check;
ALTER TABLE suggester ADD CONSTRAINT suggester_kind_check
    CHECK (kind IN ('rule', 'local', 'remote', 'match'));

-- What matching found, to say how sure it is: how many days apart the row
-- and its match are, and how alike their descriptions (thousandths). Null
-- for a row with no match, or matched before these were kept.
ALTER TABLE staged_row
    ADD COLUMN match_days       smallint CHECK (match_days >= 0),
    ADD COLUMN match_similarity smallint CHECK (match_similarity BETWEEN 0 AND 1000);

-- What a suggested decision became.
ALTER TABLE suggestion_outcome DROP CONSTRAINT suggestion_outcome_label_check;
ALTER TABLE suggestion_outcome ADD CONSTRAINT suggestion_outcome_label_check
    CHECK (label IN ('account', 'counterparty', 'category', 'purpose', 'decision'));
ALTER TABLE suggestion_outcome
    ADD COLUMN suggested_decision text
        CHECK (suggested_decision IN ('match', 'new', 'skip', 'replace')),
    ADD COLUMN committed_decision text
        CHECK (committed_decision IN ('match', 'new', 'skip', 'replace')),
    ADD CONSTRAINT suggestion_outcome_decision
        CHECK ((label = 'decision') = (suggested_decision IS NOT NULL)
               AND (label = 'decision') = (committed_decision IS NOT NULL));

-- A transfer conflict (design §6.4): a staged row you chose as a transfer to
-- one account, and a staged row in another of your accounts that fits it as
-- the other side. Neither pairs until you choose again or decide either
-- row. Goes when that row does.
ALTER TABLE staged_row
    ADD COLUMN conflict_import_id uuid,
    ADD COLUMN conflict_statement smallint,
    ADD COLUMN conflict_position  integer,
    ADD CONSTRAINT staged_row_conflict
        FOREIGN KEY (conflict_import_id, conflict_statement, conflict_position)
        REFERENCES staged_row ON DELETE SET NULL,
    ADD CONSTRAINT staged_row_conflict_whole
        CHECK ((conflict_import_id IS NULL) = (conflict_statement IS NULL)
               AND (conflict_import_id IS NULL) = (conflict_position IS NULL));

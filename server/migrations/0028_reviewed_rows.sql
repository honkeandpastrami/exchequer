-- A staged row you ticked as reviewed (design §6.7): what it showed when you
-- did (its status, transfer match, and the AI suggestions it took). The tick
-- holds while the row still shows that; NULL: not ticked.
ALTER TABLE staged_row ADD COLUMN reviewed jsonb;

-- Income and expenses of liquid accounts always say what they were (design
-- §4.1). Nothing here guesses one: label any that lack it first, and until
-- then this stops with their transactions.
DO $$
DECLARE
    missing text;
BEGIN
    SELECT string_agg(DISTINCT s.transaction_id::text, ', ') INTO missing
    FROM split s
    LEFT JOIN account f ON f.id = s.from_account_id
    LEFT JOIN account t ON t.id = s.to_account_id
    WHERE s.category_id IS NULL
      AND (
          -- One of your accounts and the outside, not a system counterparty.
          ((f.liquid AND t.id IS NULL) OR (t.liquid AND f.id IS NULL))
          AND NOT EXISTS (
              SELECT 1 FROM counterparty c
              WHERE c.id IN (s.from_counterparty_id, s.to_counterparty_id)
                AND c.system IS NOT NULL)
          -- Into or out of an investment or a loan.
          OR f.liquid <> t.liquid
      );
    IF missing IS NOT NULL THEN
        RAISE EXCEPTION 'income or expenses without a category: transactions %', missing;
    END IF;
END $$;

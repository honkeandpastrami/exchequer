-- Income and expenses of liquid accounts always say who they were for
-- (design §4.1). A blank purpose meant a default: the account's owner, or
-- the Household for money into or out of an investment or a mortgage.
-- Stated, so nothing changes meaning.
UPDATE split s SET purpose_id = a.owner_id
FROM account a
WHERE s.purpose_id IS NULL AND a.liquid
  AND ((s.from_account_id = a.id AND s.to_account_id IS NULL)
       OR (s.to_account_id = a.id AND s.from_account_id IS NULL))
  AND NOT EXISTS (
      SELECT 1 FROM counterparty c
      WHERE c.id IN (s.from_counterparty_id, s.to_counterparty_id) AND c.system IS NOT NULL);

UPDATE split s SET purpose_id = (SELECT id FROM party WHERE kind = 'household')
FROM account f, account t
WHERE s.purpose_id IS NULL AND f.id = s.from_account_id AND t.id = s.to_account_id
  AND f.liquid <> t.liquid;

//! The ledger's labeled history, as suggesters learn from it (design §8):
//! every transaction with one split between one of your accounts and the
//! outside, as an imported row becomes. Adjustments (a system counterparty,
//! such as opening balances) aren't bank rows and are left out.

use rust_decimal::Decimal;
use sqlx::PgExecutor;
use time::Date;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ExampleRow {
    pub transaction_id: Uuid,
    pub account_id: Uuid,
    pub date: Date,
    /// Signed: positive into the account.
    pub amount: Decimal,
    pub description: String,
    pub counterparty_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub purpose_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub id: Uuid,
    pub kind: String,
    pub owner_id: Uuid,
    pub archived: bool,
}

pub async fn examples(db: impl PgExecutor<'_>) -> Result<Vec<ExampleRow>, sqlx::Error> {
    sqlx::query_as!(
        ExampleRow,
        r#"SELECT t.id AS transaction_id,
                  coalesce(s.from_account_id, s.to_account_id) AS "account_id!",
                  t.date, t.description,
                  CASE WHEN s.to_account_id IS NULL THEN -s.from_quantity
                       ELSE s.to_quantity END AS "amount!",
                  coalesce(s.from_counterparty_id, s.to_counterparty_id) AS counterparty_id,
                  s.category_id, s.purpose_id
           FROM transaction t
           JOIN split s ON s.transaction_id = t.id
           LEFT JOIN counterparty c
                  ON c.id = coalesce(s.from_counterparty_id, s.to_counterparty_id)
           WHERE (s.from_account_id IS NULL) <> (s.to_account_id IS NULL)
             AND s.from_commodity_id = s.to_commodity_id
             AND c.system IS NULL
             AND NOT EXISTS (SELECT 1 FROM split x
                             WHERE x.transaction_id = t.id AND x.position <> s.position)
           ORDER BY t.date, t.id"#
    )
    .fetch_all(db)
    .await
}

pub async fn accounts(db: impl PgExecutor<'_>) -> Result<Vec<AccountRow>, sqlx::Error> {
    sqlx::query_as!(
        AccountRow,
        "SELECT id, kind, owner_id, archived FROM account ORDER BY id"
    )
    .fetch_all(db)
    .await
}

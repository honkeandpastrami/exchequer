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

/// What remote AI may name and describe (design §8): accounts with their
/// owners and numbers, ordinary counterparties, categories and parties.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub accounts: Vec<CatalogAccount>,
    pub counterparties: Vec<(Uuid, String)>,
    pub categories: Vec<(Uuid, String)>,
    /// `(id, name, kind)`.
    pub parties: Vec<(Uuid, String, String)>,
}

#[derive(Debug, Clone)]
pub struct CatalogAccount {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub owner_id: Uuid,
    pub archived: bool,
    /// The numbers it's known by, whole: callers keep only the last four.
    pub numbers: Vec<domain::AccountNumber>,
}

pub async fn catalog(db: &mut sqlx::PgConnection) -> Result<Catalog, sqlx::Error> {
    let numbers = sqlx::query!("SELECT account_id, number FROM account_number ORDER BY number")
        .fetch_all(&mut *db)
        .await?;
    let accounts = sqlx::query!(
        "SELECT id, name, kind, owner_id, archived FROM account ORDER BY owner_id, name"
    )
    .fetch_all(&mut *db)
    .await?
    .into_iter()
    .map(|a| CatalogAccount {
        numbers: numbers
            .iter()
            .filter(|n| n.account_id == a.id)
            .filter_map(|n| domain::AccountNumber::new(&n.number))
            .collect(),
        id: a.id,
        name: a.name,
        kind: a.kind,
        owner_id: a.owner_id,
        archived: a.archived,
    })
    .collect();
    let counterparties =
        sqlx::query!("SELECT id, name FROM counterparty WHERE system IS NULL ORDER BY name")
            .fetch_all(&mut *db)
            .await?
            .into_iter()
            .map(|r| (r.id, r.name))
            .collect();
    let categories = sqlx::query!("SELECT id, name FROM category ORDER BY name")
        .fetch_all(&mut *db)
        .await?
        .into_iter()
        .map(|r| (r.id, r.name))
        .collect();
    let parties = sqlx::query!("SELECT id, name, kind FROM party ORDER BY name")
        .fetch_all(&mut *db)
        .await?
        .into_iter()
        .map(|r| (r.id, r.name, r.kind))
        .collect();
    Ok(Catalog {
        accounts,
        counterparties,
        categories,
        parties,
    })
}

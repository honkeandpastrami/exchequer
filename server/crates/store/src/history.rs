//! The ledger's labeled history, as suggesters learn from it (design §8):
//! every transaction with one split between one of your accounts and the
//! outside, as an imported row becomes, and every one-split transfer between
//! two of your accounts, once from each side. Adjustments (a system
//! counterparty, such as opening balances) aren't bank rows and are left
//! out.

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
    /// For a transfer: the other account.
    pub transfer_account_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub purpose_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub id: Uuid,
    pub kind: String,
    pub owner_id: Uuid,
    pub liquid: bool,
    pub archived: bool,
}

pub async fn examples(db: impl PgExecutor<'_>) -> Result<Vec<ExampleRow>, sqlx::Error> {
    sqlx::query_as!(
        ExampleRow,
        r#"WITH one AS (
               SELECT t.id, t.date, t.description, s.from_account_id, s.to_account_id,
                      s.from_counterparty_id, s.to_counterparty_id, s.from_quantity,
                      s.to_quantity, s.category_id, s.purpose_id
               FROM transaction t
               JOIN split s ON s.transaction_id = t.id
               WHERE s.from_commodity_id = s.to_commodity_id
                 AND NOT EXISTS (SELECT 1 FROM split x
                                 WHERE x.transaction_id = t.id AND x.position <> s.position))
           SELECT o.id AS "transaction_id!",
                  coalesce(o.from_account_id, o.to_account_id) AS "account_id!",
                  o.date AS "date!", o.description AS "description!",
                  CASE WHEN o.to_account_id IS NULL THEN -o.from_quantity
                       ELSE o.to_quantity END AS "amount!",
                  coalesce(o.from_counterparty_id, o.to_counterparty_id) AS counterparty_id,
                  NULL::uuid AS transfer_account_id,
                  o.category_id, o.purpose_id
           FROM one o
           LEFT JOIN counterparty c
                  ON c.id = coalesce(o.from_counterparty_id, o.to_counterparty_id)
           WHERE (o.from_account_id IS NULL) <> (o.to_account_id IS NULL)
             AND c.system IS NULL
           UNION ALL
           SELECT o.id, o.from_account_id, o.date, o.description, -o.from_quantity,
                  NULL, o.to_account_id, o.category_id, o.purpose_id
           FROM one o
           WHERE o.from_account_id IS NOT NULL AND o.to_account_id IS NOT NULL
           UNION ALL
           SELECT o.id, o.to_account_id, o.date, o.description, o.to_quantity,
                  NULL, o.from_account_id, o.category_id, o.purpose_id
           FROM one o
           WHERE o.from_account_id IS NOT NULL AND o.to_account_id IS NOT NULL
           ORDER BY 3, 1, 2"#
    )
    .fetch_all(db)
    .await
}

pub async fn accounts(db: impl PgExecutor<'_>) -> Result<Vec<AccountRow>, sqlx::Error> {
    sqlx::query_as!(
        AccountRow,
        "SELECT id, kind, owner_id, liquid, archived FROM account ORDER BY id"
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

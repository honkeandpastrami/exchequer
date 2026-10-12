//! Account numbers bank files name accounts by (design §6.2). Full numbers
//! go in and out of here only as `AccountNumber`, which masks itself.

use domain::AccountNumber;
use sqlx::PgExecutor;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct NumberRow {
    pub id: Uuid,
    pub account_id: Uuid,
    pub number: AccountNumber,
}

struct Raw {
    id: Uuid,
    account_id: Uuid,
    number: String,
}

fn row(r: Raw) -> Option<NumberRow> {
    Some(NumberRow {
        id: r.id,
        account_id: r.account_id,
        number: AccountNumber::new(&r.number)?,
    })
}

/// The numbers of every account in `accounts`, in the order they were added.
pub async fn of(db: impl PgExecutor<'_>, accounts: &[Uuid]) -> Result<Vec<NumberRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        Raw,
        "SELECT id, account_id, number FROM account_number
         WHERE account_id = ANY($1) ORDER BY id",
        accounts
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().filter_map(row).collect())
}

/// The account a number belongs to, if any.
pub async fn find(
    db: impl PgExecutor<'_>,
    number: &AccountNumber,
) -> Result<Option<NumberRow>, sqlx::Error> {
    let r = sqlx::query_as!(
        Raw,
        "SELECT id, account_id, number FROM account_number WHERE number = $1",
        number.reveal()
    )
    .fetch_optional(db)
    .await?;
    Ok(r.and_then(row))
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    id: Uuid,
    account_id: Uuid,
    number: &AccountNumber,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO account_number (id, account_id, number) VALUES ($1, $2, $3)",
        id,
        account_id,
        number.reveal()
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Whether the account had that number.
pub async fn delete(
    db: impl PgExecutor<'_>,
    account_id: Uuid,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query!(
        "DELETE FROM account_number WHERE id = $1 AND account_id = $2",
        id,
        account_id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() > 0)
}

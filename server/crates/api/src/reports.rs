//! `/api/v1/reports` (design §7): named reports built on the query engine
//! and the settlement. Others (income vs. expense, spending by category or
//! tag, balance history) are single `/query` calls.

use std::collections::BTreeMap;

use axum::extract::State;
use domain::{Amount, Month};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use time::Date;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::problem::{Path, Problem, Query};
use crate::query::{GroupBy, KindName, Measure, QueryRequest, Row, run};
use crate::settlements::{compute, household, money};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(balances))
        .routes(routes!(household_pool))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct BalancesParams {
    /// Balances at the end of this day.
    date: Date,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccountBalance {
    account_id: Uuid,
    commodity_id: Uuid,
    /// At the commodity's precision.
    #[schema(value_type = String)]
    balance: Amount,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Balances {
    date: Date,
    /// Every account and commodity whose balance on the date isn't zero.
    balances: Vec<AccountBalance>,
}

/// Each account's balance in each commodity at the end of a day.
#[utoipa::path(get, path = "/reports/balances", tag = "reports", security(("bearer" = [])),
    params(BalancesParams),
    responses((status = OK, body = Balances),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn balances(
    State(state): State<AppState>,
    Query(p): Query<BalancesParams>,
) -> Result<axum::Json<Balances>, Problem> {
    let mut conn = state.pool.acquire().await?;
    let rows = run(
        &mut conn,
        &QueryRequest {
            measure: Measure::Balance,
            to: Some(p.date),
            group_by: vec![GroupBy::Account],
            ..QueryRequest::default()
        },
    )
    .await?;
    Ok(axum::Json(Balances {
        date: p.date,
        balances: rows
            .into_iter()
            .map(|r| AccountBalance {
                account_id: r.account_id.unwrap_or_default(),
                commodity_id: r.commodity_id,
                balance: r.amount,
            })
            .collect(),
    }))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CategoryAmount {
    category_id: Option<Uuid>,
    #[schema(value_type = String)]
    amount: Amount,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PartyAmount {
    party_id: Uuid,
    #[schema(value_type = String)]
    amount: Amount,
}

/// The household's month (design §4.3), in USD. Amounts in other
/// commodities aren't included until prices exist.
#[derive(Debug, Serialize, ToSchema)]
pub struct HouseholdPool {
    #[schema(value_type = String, example = "2026-06")]
    month: Month,
    currency_id: Uuid,
    /// Income for the household, by category (positive).
    income: Vec<CategoryAmount>,
    /// Investment proceeds that became shared (positive).
    #[schema(value_type = String)]
    liquidations: Amount,
    /// Spending for the household, by category (negative).
    expenses: Vec<CategoryAmount>,
    /// What the household owes each person for the month.
    stipends: Vec<PartyAmount>,
    /// How much each business's net funding grew this month.
    business_funding: Vec<PartyAmount>,
    /// The change in the household's own liquid accounts over the month.
    #[schema(value_type = String)]
    shared_savings_change: Amount,
}

async fn flow(
    db: &mut PgConnection,
    req: QueryRequest,
    currency: Uuid,
) -> Result<Vec<Row>, Problem> {
    Ok(run(db, &req)
        .await?
        .into_iter()
        .filter(|r| r.commodity_id == currency)
        .collect())
}

/// The household pool for one month: shared income and expenses, stipends,
/// business funding, and the change in shared savings.
#[utoipa::path(get, path = "/reports/household-pool/{month}", tag = "reports", security(("bearer" = [])),
    params(("month" = String, Path, description = "`YYYY-MM`")),
    responses((status = OK, body = HouseholdPool),
              (status = CONFLICT, description = "No USD commodity yet", body = Problem, content_type = "application/problem+json"),
              (status = UNPROCESSABLE_ENTITY, body = Problem, content_type = "application/problem+json")))]
async fn household_pool(
    State(state): State<AppState>,
    Path(month): Path<Month>,
) -> Result<axum::Json<HouseholdPool>, Problem> {
    let mut conn = state.pool.acquire().await?;
    let db = &mut *conn;
    let (household_id, currency) = household(db).await?;
    let (household_id, currency) = (*household_id.as_uuid(), *currency.as_uuid());
    let in_month = QueryRequest {
        from: Some(month.first_day()),
        to: Some(month.last_day()),
        purpose_ids: vec![household_id],
        ..QueryRequest::default()
    };

    let mut income = Vec::new();
    let mut expenses = Vec::new();
    let rows = flow(
        db,
        QueryRequest {
            kinds: vec![KindName::Income, KindName::Expense],
            group_by: vec![GroupBy::Kind, GroupBy::Category],
            ..in_month.clone()
        },
        currency,
    )
    .await?;
    for r in rows {
        let line = CategoryAmount {
            category_id: r.category_id.flatten(),
            amount: r.amount,
        };
        match r.kind {
            Some(KindName::Income) => income.push(line),
            _ => expenses.push(line),
        }
    }

    // Liquidations: only the side landing in a liquid account counts.
    let accounts = store::accounts::list(&mut *db, None, i64::MAX).await?;
    let liquid: Vec<Uuid> = accounts.iter().filter(|a| a.liquid).map(|a| a.id).collect();
    let shared: Vec<Uuid> = accounts
        .iter()
        .filter(|a| a.liquid && a.owner_id == household_id)
        .map(|a| a.id)
        .collect();
    let c = compute(db, month).await?;
    let m = |a: Amount| money(a, c.scale);
    let liquidations = if liquid.is_empty() {
        Amount::ZERO
    } else {
        flow(
            db,
            QueryRequest {
                kinds: vec![KindName::Transfer],
                account_ids: liquid,
                ..in_month.clone()
            },
            currency,
        )
        .await?
        .into_iter()
        .map(|r| r.amount)
        .next()
        .unwrap_or_default()
    };

    let balance = async |db: &mut PgConnection, day: Date| -> Result<Amount, Problem> {
        if shared.is_empty() {
            return Ok(Amount::ZERO);
        }
        let rows = flow(
            db,
            QueryRequest {
                measure: Measure::Balance,
                to: Some(day),
                account_ids: shared.clone(),
                ..QueryRequest::default()
            },
            currency,
        )
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| r.amount)
            .next()
            .unwrap_or_default())
    };
    let end = balance(db, month.last_day()).await?;
    let start = balance(db, month.previous().last_day()).await?;
    let shared_savings_change = m(end.checked_sub(start).map_err(|_| Problem::internal())?);

    let now = c
        .settlement
        .statement(month)
        .map_err(|_| Problem::internal())?;
    let before = c
        .settlement
        .statement(month.previous())
        .map_err(|_| Problem::internal())?;
    let prior: BTreeMap<Uuid, Amount> = before
        .edges
        .iter()
        .map(|e| (*e.child.as_uuid(), e.outstanding))
        .collect();
    let kinds: BTreeMap<Uuid, String> = store::parties::all(&mut *db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.kind))
        .collect();
    let mut stipends = Vec::new();
    let mut business_funding = Vec::new();
    for e in &now.edges {
        let id = *e.child.as_uuid();
        match kinds.get(&id).map(String::as_str) {
            Some("person") => stipends.push(PartyAmount {
                party_id: id,
                amount: m(e.stipend),
            }),
            Some("business") => {
                let was = prior.get(&id).copied().unwrap_or_default();
                let grew = e
                    .outstanding
                    .checked_sub(was)
                    .map_err(|_| Problem::internal())?;
                business_funding.push(PartyAmount {
                    party_id: id,
                    amount: m(grew),
                });
            }
            _ => {}
        }
    }

    Ok(axum::Json(HouseholdPool {
        month,
        currency_id: currency,
        income,
        liquidations: m(liquidations),
        expenses,
        stipends,
        business_funding,
        shared_savings_change,
    }))
}

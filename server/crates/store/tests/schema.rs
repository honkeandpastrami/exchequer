//! Schema-level guarantees, checked against a fresh database per test.
//! Needs the dev database: `just db-up`.

use sqlx::PgPool;

/// Design §3.5: no column anywhere may be floating point.
#[sqlx::test(migrator = "store::MIGRATOR")]
async fn no_floating_point_columns(pool: PgPool) {
    let floats: Vec<(String, String)> = sqlx::query_as(
        "SELECT table_name::text, column_name::text FROM information_schema.columns
         WHERE table_schema = 'public' AND data_type IN ('real', 'double precision')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(floats.is_empty(), "floating-point columns: {floats:?}");
}

/// NUMERIC keeps exactly what it is given: values that break floats survive
/// a round trip unchanged.
#[sqlx::test(migrator = "store::MIGRATOR")]
async fn numeric_round_trips_exactly(pool: PgPool) {
    for v in ["0.3", "1234.5699999999999", "99999999999999999.99", "4.125"] {
        let (back,): (String,) = sqlx::query_as("SELECT ($1::numeric + 0)::text")
            .bind(v)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(back, v);
    }
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn system_counterparties_exist(pool: PgPool) {
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM counterparty WHERE system IS NOT NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 2);
}

#[sqlx::test(migrator = "store::MIGRATOR")]
async fn only_one_household(pool: PgPool) {
    let insert = "INSERT INTO party (id, name, kind) VALUES (gen_random_uuid(), $1, 'household')";
    sqlx::query(insert)
        .bind("Household")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query(insert)
            .bind("Another")
            .execute(&pool)
            .await
            .is_err()
    );
}

/// The split CHECK constraints reject what `domain` rejects, even if a bug
/// lets an invalid split through.
#[sqlx::test(migrator = "store::MIGRATOR")]
async fn split_constraints(pool: PgPool) {
    sqlx::raw_sql(
        "INSERT INTO party VALUES ('00000000-0000-0000-0000-000000000001', 'Alex', 'person');
         INSERT INTO commodity VALUES
             ('00000000-0000-0000-0000-0000000000a1', 'USD', 'fiat', 2),
             ('00000000-0000-0000-0000-0000000000a2', 'EUR', 'fiat', 2);
         INSERT INTO account VALUES
             ('00000000-0000-0000-0000-0000000000b1', 'Alex Checking', 'checking',
              '00000000-0000-0000-0000-000000000001', true),
             ('00000000-0000-0000-0000-0000000000b2', 'Alex Credit', 'credit_card',
              '00000000-0000-0000-0000-000000000001', true);
         INSERT INTO counterparty VALUES
             ('00000000-0000-0000-0000-0000000000c1', 'Grocer', NULL),
             ('00000000-0000-0000-0000-0000000000c2', 'Store', NULL);
         INSERT INTO transaction (id, date, description)
             VALUES ('00000000-0000-0000-0000-0000000000d1', '2026-03-14', 'Test');",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (usd, eur) = ("a1", "a2");
    let (checking, credit) = ("b1", "b2");
    let (grocer, store) = ("c1", "c2");
    // (from account, from counterparty, from commodity, qty, to account, to counterparty, to commodity, qty)
    type Row<'a> = (
        Option<&'a str>,
        Option<&'a str>,
        &'a str,
        &'a str,
        Option<&'a str>,
        Option<&'a str>,
        &'a str,
        &'a str,
    );
    let ok: &[Row] = &[
        (Some(credit), None, usd, "80", None, Some(grocer), usd, "80"),
        (
            Some(checking),
            None,
            usd,
            "500",
            Some(credit),
            None,
            usd,
            "500.00",
        ),
        (
            Some(checking),
            None,
            usd,
            "1100",
            Some(checking),
            None,
            eur,
            "1000",
        ),
    ];
    let rejected: &[Row] = &[
        (None, Some(store), usd, "10", None, Some(grocer), usd, "10"),
        (
            Some(checking),
            None,
            usd,
            "10",
            Some(checking),
            None,
            usd,
            "10",
        ),
        (
            Some(checking),
            None,
            usd,
            "10",
            Some(credit),
            None,
            usd,
            "9.99",
        ),
        (Some(credit), None, usd, "0", None, Some(grocer), usd, "0"),
        (Some(credit), None, usd, "-5", None, Some(grocer), usd, "-5"),
        (
            Some(credit),
            Some(store),
            usd,
            "5",
            None,
            Some(grocer),
            usd,
            "5",
        ),
        (None, None, usd, "5", None, Some(grocer), usd, "5"),
    ];

    let id = |s: Option<&str>| s.map(|s| format!("00000000-0000-0000-0000-0000000000{s}"));
    let mut position = 0i16;
    for (rows, should_pass) in [(ok, true), (rejected, false)] {
        for r in rows {
            position += 1;
            let result = sqlx::query(
                "INSERT INTO split (transaction_id, position,
                     from_account_id, from_counterparty_id, from_commodity_id, from_quantity,
                     to_account_id, to_counterparty_id, to_commodity_id, to_quantity)
                 VALUES ('00000000-0000-0000-0000-0000000000d1', $1,
                     $2::uuid, $3::uuid, $4::uuid, $5::numeric,
                     $6::uuid, $7::uuid, $8::uuid, $9::numeric)",
            )
            .bind(position)
            .bind(id(r.0))
            .bind(id(r.1))
            .bind(id(Some(r.2)))
            .bind(r.3)
            .bind(id(r.4))
            .bind(id(r.5))
            .bind(id(Some(r.6)))
            .bind(r.7)
            .execute(&pool)
            .await;
            assert_eq!(result.is_ok(), should_pass, "{r:?}: {result:?}");
        }
    }
}

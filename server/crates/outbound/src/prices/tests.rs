use time::macros::date;

use super::*;
use crate::test_server::serve;

fn pair(base: &str, quote: &str, kind: Kind) -> Pair {
    Pair {
        base: base.into(),
        quote: quote.into(),
        kind,
    }
}

#[test]
fn tiingo_plans_one_pair() {
    let t = Tiingo::new(Tiingo::URL, "k", Duration::from_secs(1)).unwrap();
    let c = Kind::Currency;
    assert_eq!(
        t.plan("GBP", c, "USD", c),
        Some(vec![pair("GBP", "USD", c)])
    );
    assert_eq!(
        t.plan("VTI", Kind::Listed, "USD", c),
        Some(vec![pair("VTI", "USD", Kind::Listed)])
    );
    assert_eq!(
        t.plan("USD", c, "VTI", Kind::Listed),
        Some(vec![pair("VTI", "USD", Kind::Listed)])
    );
    assert_eq!(t.plan("VTI", Kind::Listed, "EUR", c), None);
    assert_eq!(t.plan("ACME", Kind::Other, "USD", c), None);
}

#[test]
fn ecb_plans_through_the_euro() {
    let e = Ecb::new(Ecb::URL, Duration::from_secs(1)).unwrap();
    let c = Kind::Currency;
    assert_eq!(
        e.plan("EUR", c, "USD", c),
        Some(vec![pair("EUR", "USD", c)])
    );
    assert_eq!(
        e.plan("USD", c, "EUR", c),
        Some(vec![pair("EUR", "USD", c)])
    );
    assert_eq!(
        e.plan("GBP", c, "USD", c),
        Some(vec![pair("EUR", "GBP", c), pair("EUR", "USD", c)])
    );
    assert_eq!(e.plan("VTI", Kind::Listed, "USD", c), None);
}

#[test]
fn tiingo_closes_keep_their_digits() {
    let body = r#"[{"date":"2026-06-01T00:00:00.000Z","close":1.08420,"open":1.1},
                   {"date":"2026-06-02T00:00:00.000Z","close":0.1},
                   {"date":"2026-06-03T00:00:00.000Z","close":null}]"#;
    let q = parse_tiingo(&pair("EUR", "USD", Kind::Currency), body).unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!(
        (q[0].date, q[0].rate.as_str()),
        (date!(2026 - 06 - 01), "1.08420")
    );
    assert_eq!(q[1].rate, "0.1");
    for bad in [
        r#"[{"date":"x","close":1}]"#,
        r#"[{"date":"2026-06-01","close":1e3}]"#,
        "{}",
    ] {
        assert!(
            parse_tiingo(&pair("EUR", "USD", Kind::Currency), bad).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn tiingo_asks_each_pair_with_its_key() {
    let (url, server) = serve(vec![
        (
            200,
            "",
            r#"[{"date":"2026-06-01T00:00:00.000Z","close":250.5}]"#,
        ),
        (404, "", r#"{"detail":"Error: Ticker 'NOPE' not found"}"#),
        (
            200,
            "",
            r#"[{"date":"2026-06-01T00:00:00+00:00","close":1.25}]"#,
        ),
    ]);
    let t = Tiingo::new(&url, "s3cret", Duration::from_secs(5)).unwrap();
    let q = t
        .fetch(
            &[
                pair("BRK.B", "USD", Kind::Listed),
                pair("NOPE", "USD", Kind::Listed),
                pair("GBP", "USD", Kind::Currency),
            ],
            date!(2026 - 05 - 28),
            date!(2026 - 06 - 01),
        )
        .unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!((q[0].base.as_str(), q[0].rate.as_str()), ("BRK.B", "250.5"));
    assert_eq!((q[1].base.as_str(), q[1].quote.as_str()), ("GBP", "USD"));
    let seen = server.join().unwrap();
    assert!(
        seen[0]
            .0
            .starts_with("GET /tiingo/daily/brk-b/prices?startDate=2026-05-28&endDate=2026-06-01 "),
        "{}",
        seen[0].0
    );
    assert!(seen[0].0.contains("authorization: Token s3cret\r\n"));
    assert!(
        seen[2]
            .0
            .starts_with("GET /tiingo/fx/gbpusd/prices?startDate=2026-05-28&endDate=2026-06-01&resampleFreq=1day "),
        "{}",
        seen[2].0
    );
}

#[test]
fn tiingo_errors() {
    let (url, _server) = serve(vec![
        (429, "", "{}"),
        (401, "", r#"{"detail":"Invalid token."}"#),
    ]);
    let t = Tiingo::new(&url, "k", Duration::from_secs(5)).unwrap();
    let one = [pair("VTI", "USD", Kind::Listed)];
    let (d1, d2) = (date!(2026 - 06 - 01), date!(2026 - 06 - 01));
    assert!(matches!(t.fetch(&one, d1, d2), Err(Error::RateLimited)));
    let e = t.fetch(&one, d1, d2).unwrap_err();
    assert_eq!(e.to_string(), "HTTP 401: Invalid token.");
    assert!(Tiingo::new("http://example.com", "k", Duration::from_secs(1)).is_err());
    assert!(Tiingo::new(Tiingo::URL, "", Duration::from_secs(1)).is_err());
}

const ECB: &str =
    "KEY,FREQ,CURRENCY,CURRENCY_DENOM,EXR_TYPE,EXR_SUFFIX,TIME_PERIOD,OBS_VALUE,OBS_STATUS,TITLE
EXR.D.GBP.EUR.SP00.A,D,GBP,EUR,SP00,A,2026-06-01,0.84478,A,\"Pound sterling/Euro, 2.15 pm\"
EXR.D.USD.EUR.SP00.A,D,USD,EUR,SP00,A,2026-06-01,1.1612,A,\"US dollar/Euro, 2.15 pm\"
EXR.D.USD.EUR.SP00.A,D,USD,EUR,SP00,A,2026-06-02,,M,\"US dollar/Euro, 2.15 pm\"
";

#[test]
fn ecb_rates_are_against_the_euro() {
    let q = parse_ecb(ECB).unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!(
        (q[0].base.as_str(), q[0].quote.as_str(), q[0].rate.as_str()),
        ("EUR", "GBP", "0.84478")
    );
    assert_eq!(q[1].date, date!(2026 - 06 - 01));
    assert!(parse_ecb("").unwrap().is_empty());
    assert!(parse_ecb("A,B\n1,2\n").is_err());
}

#[test]
fn ecb_asks_once_for_every_currency() {
    let (url, server) = serve(vec![(200, "", ECB), (404, "", "No results found.")]);
    let e = Ecb::new(&url, Duration::from_secs(5)).unwrap();
    let c = Kind::Currency;
    let pairs = [
        pair("EUR", "USD", c),
        pair("EUR", "GBP", c),
        pair("EUR", "USD", c),
    ];
    let q = e
        .fetch(&pairs, date!(2026 - 06 - 01), date!(2026 - 06 - 02))
        .unwrap();
    assert_eq!(q.len(), 2);
    assert!(
        e.fetch(&pairs, date!(2026 - 06 - 06), date!(2026 - 06 - 07))
            .unwrap()
            .is_empty()
    );
    let seen = server.join().unwrap();
    assert!(
        seen[0].0.starts_with(
            "GET /service/data/EXR/D.GBP+USD.EUR.SP00.A?startPeriod=2026-06-01&endPeriod=2026-06-02&format=csvdata "
        ),
        "{}",
        seen[0].0
    );
}

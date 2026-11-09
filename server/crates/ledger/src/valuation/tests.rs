use time::macros::date;

use super::*;

fn r(s: &str) -> Rate {
    s.parse().unwrap()
}

struct Ids {
    usd: CommodityId,
    eur: CommodityId,
    gbp: CommodityId,
    vti: CommodityId,
}

fn ids() -> Ids {
    Ids {
        usd: CommodityId::new(),
        eur: CommodityId::new(),
        gbp: CommodityId::new(),
        vti: CommodityId::new(),
    }
}

fn p(base: CommodityId, quote: CommodityId, date: Date, rate: &str, source: &str) -> Price {
    Price {
        base,
        quote,
        date,
        rate: r(rate),
        source: Source::parse(source).unwrap(),
        provisional: false,
    }
}

fn prices(list: Vec<Price>) -> Prices {
    Prices::new(list, vec!["tiingo".into(), "ecb".into()], 5)
}

fn sources(v: &Valuation) -> Vec<String> {
    v.used.iter().map(|u| u.price.source.to_string()).collect()
}

#[test]
fn sources_parse_and_print() {
    for s in ["manual", "provider:tiingo", "provider:ecb", "provider:a-1"] {
        assert_eq!(Source::parse(s).unwrap().to_string(), s);
    }
    for bad in ["", "provider:", "provider:Tiingo", "tiingo", "implied"] {
        assert!(Source::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn same_commodity_is_one() {
    let c = ids();
    let v = prices(vec![])
        .value(c.usd, c.usd, date!(2026 - 06 - 01))
        .unwrap();
    assert_eq!(v.rate, Rate::one());
    assert!(v.used.is_empty() && !v.stale);
}

#[test]
fn latest_on_or_before_the_date() {
    let c = ids();
    let ps = prices(vec![
        p(
            c.vti,
            c.usd,
            date!(2026 - 06 - 01),
            "300",
            "provider:tiingo",
        ),
        p(
            c.vti,
            c.usd,
            date!(2026 - 06 - 03),
            "310",
            "provider:tiingo",
        ),
        p(
            c.vti,
            c.usd,
            date!(2026 - 06 - 05),
            "320",
            "provider:tiingo",
        ),
    ]);
    let v = ps.value(c.vti, c.usd, date!(2026 - 06 - 04)).unwrap();
    assert_eq!(v.rate, r("310"));
    assert!(!v.stale && !v.derived());
    assert!(ps.value(c.vti, c.usd, date!(2026 - 05 - 31)).is_none());
}

#[test]
fn inverse_of_one_price() {
    let c = ids();
    let ps = prices(vec![p(
        c.eur,
        c.usd,
        date!(2026 - 06 - 01),
        "1.25",
        "provider:ecb",
    )]);
    let v = ps.value(c.usd, c.eur, date!(2026 - 06 - 01)).unwrap();
    assert_eq!(v.rate, r("0.8"));
    assert!(v.used[0].inverted && !v.derived());
}

#[test]
fn manual_wins_then_source_order() {
    let c = ids();
    let d = date!(2026 - 06 - 01);
    let all = vec![
        p(c.eur, c.usd, d, "1.10", "provider:ecb"),
        p(c.eur, c.usd, d, "1.11", "provider:tiingo"),
        p(c.eur, c.usd, d, "1.12", "manual"),
    ];
    assert_eq!(
        prices(all.clone()).value(c.eur, c.usd, d).unwrap().rate,
        r("1.12")
    );
    assert_eq!(
        prices(all[..2].to_vec())
            .value(c.eur, c.usd, d)
            .unwrap()
            .rate,
        r("1.11")
    );
    // A source no longer configured still counts, after the others.
    let gone = vec![p(c.eur, c.usd, d, "1.09", "provider:old"), all[0].clone()];
    assert_eq!(
        prices(gone.clone()).value(c.eur, c.usd, d).unwrap().rate,
        r("1.10")
    );
    assert_eq!(
        prices(gone[..1].to_vec())
            .value(c.eur, c.usd, d)
            .unwrap()
            .rate,
        r("1.09")
    );
}

#[test]
fn one_price_beats_a_chain() {
    let c = ids();
    let d = date!(2026 - 06 - 01);
    let ps = prices(vec![
        p(c.eur, c.usd, d, "1.25", "provider:ecb"),
        p(c.eur, c.gbp, d, "0.80", "provider:ecb"),
        p(c.gbp, c.usd, d, "1.50", "provider:tiingo"),
    ]);
    let v = ps.value(c.gbp, c.usd, d).unwrap();
    assert_eq!(v.rate, r("1.50"));
    assert_eq!(sources(&v), ["provider:tiingo"]);
}

#[test]
fn a_chain_within_one_source_when_no_price_is_direct() {
    let c = ids();
    let d = date!(2026 - 06 - 01);
    let ps = prices(vec![
        p(c.eur, c.usd, d, "1.25", "provider:ecb"),
        p(c.eur, c.gbp, d, "0.80", "provider:ecb"),
        // Preferred source, but mixing sources is the last resort.
        p(c.eur, c.gbp, d, "0.81", "provider:tiingo"),
    ]);
    let v = ps.value(c.gbp, c.usd, d).unwrap();
    // 1 GBP = 1/0.80 EUR = 1.25 EUR; 1 EUR = 1.25 USD.
    assert_eq!(v.rate, r("1.5625"));
    assert!(v.derived() && !v.stale);
    assert_eq!(sources(&v), ["provider:ecb", "provider:ecb"]);
    assert!(v.used[0].inverted && !v.used[1].inverted);
}

#[test]
fn mixed_chain_as_last_resort() {
    let c = ids();
    let d = date!(2026 - 06 - 01);
    let ps = prices(vec![
        p(c.vti, c.eur, d, "250", "manual"),
        p(c.eur, c.usd, d, "1.2", "provider:ecb"),
    ]);
    let v = ps.value(c.vti, c.usd, d).unwrap();
    assert_eq!(v.rate, r("300"));
    assert_eq!(sources(&v), ["manual", "provider:ecb"]);
}

#[test]
fn fresh_beats_stale_and_stale_says_so() {
    let c = ids();
    let ps = prices(vec![
        p(
            c.gbp,
            c.usd,
            date!(2026 - 05 - 01),
            "1.40",
            "provider:tiingo",
        ),
        p(c.eur, c.usd, date!(2026 - 06 - 01), "1.25", "provider:ecb"),
        p(c.eur, c.gbp, date!(2026 - 06 - 01), "0.80", "provider:ecb"),
    ]);
    let v = ps.value(c.gbp, c.usd, date!(2026 - 06 - 02)).unwrap();
    assert!(v.derived() && !v.stale);
    assert_eq!(v.rate, r("1.5625"));

    // Nothing fresh: the newest figure, marked stale.
    let v = ps.value(c.gbp, c.usd, date!(2026 - 07 - 01)).unwrap();
    assert!(v.stale && v.derived());
    let v = ps.value(c.gbp, c.usd, date!(2026 - 05 - 20)).unwrap();
    assert!(v.stale && !v.derived());
    assert_eq!(v.rate, r("1.40"));
}

#[test]
fn a_weekend_is_not_stale() {
    let c = ids();
    let ps = prices(vec![p(
        c.vti,
        c.usd,
        date!(2026 - 06 - 05),
        "300",
        "provider:tiingo",
    )]);
    assert!(!ps.value(c.vti, c.usd, date!(2026 - 06 - 08)).unwrap().stale);
    assert!(ps.value(c.vti, c.usd, date!(2026 - 06 - 11)).unwrap().stale);
}

#[test]
fn checks() {
    let c = ids();
    let d = date!(2026 - 06 - 01);
    assert!(p(c.eur, c.usd, d, "1.1", "manual").check().is_ok());
    assert_eq!(
        p(c.eur, c.eur, d, "1", "manual").check(),
        Err(PriceError::SameCommodity)
    );
    assert_eq!(
        p(c.eur, c.usd, d, "0", "manual").check(),
        Err(PriceError::NotPositive)
    );
    assert_eq!(
        p(c.eur, c.usd, d, "0.000000000000000000001", "manual").check(),
        Err(PriceError::TooPrecise)
    );
    assert!(
        p(c.eur, c.usd, d, "1.100000000000000000000", "manual")
            .check()
            .is_ok()
    );
}

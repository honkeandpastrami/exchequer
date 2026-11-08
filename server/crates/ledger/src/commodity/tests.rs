use super::*;

fn usd() -> Commodity {
    Commodity::new(CommodityId::new(), "USD", CommodityKind::Fiat, 2).unwrap()
}

fn a(s: &str) -> Amount {
    s.parse().unwrap()
}

#[test]
fn codes() {
    let id = CommodityId::new();
    for ok in ["USD", "VTI", "BRK.B", "BTC", "X"] {
        assert!(
            Commodity::new(id, ok, CommodityKind::Other, 2).is_ok(),
            "{ok}"
        );
    }
    for bad in ["", "usd", "US D", "€", "ABCDEFGHIJKLMNOPQ"] {
        assert!(
            Commodity::new(id, bad, CommodityKind::Other, 2).is_err(),
            "{bad}"
        );
    }
    assert!(Commodity::new(id, "X", CommodityKind::Other, 19).is_err());
}

#[test]
fn precision_is_enforced_without_rounding() {
    let usd = usd();
    assert!(usd.check(a("80")).is_ok());
    assert!(usd.check(a("80.10")).is_ok());
    assert!(usd.check(a("80.000")).is_ok());
    assert!(usd.check(a("80.005")).is_err());
    // A float-damaged value exported by another tool.
    assert!(usd.check(a("1234.5699999999999")).is_err());

    let jpy = Commodity::new(CommodityId::new(), "JPY", CommodityKind::Fiat, 0).unwrap();
    assert!(jpy.check(a("1500")).is_ok());
    assert!(jpy.check(a("1500.5")).is_err());

    let shares = Commodity::new(CommodityId::new(), "VTI", CommodityKind::Fund, 3).unwrap();
    assert!(shares.check(a("4.125")).is_ok());
}

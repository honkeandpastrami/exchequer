use super::*;
use time::macros::date;

fn row(amount: &str, description: &str, native: Option<&str>) -> Row {
    Row {
        date: date!(2026 - 03 - 02),
        amount: amount.parse().unwrap(),
        description: description.to_owned(),
        native_id: native.map(str::to_owned),
        raw: serde_json::Map::new(),
    }
}

#[test]
fn same_ids_as_the_preimporter() {
    // hashlib.sha256(b"2026-03-02GROCER #12-80.00").hexdigest()[:15]
    let expected = {
        let d = Sha256::digest(b"2026-03-02GROCER #12-80.00");
        d.iter().map(|b| format!("{b:02x}")).collect::<String>()[..15].to_owned()
    };
    let rows = [
        row("-80", "GROCER #12", None),
        row("-80.00", "GROCER #12", None),
        row("-80.001", "OTHER", None),
        row("5", "X", Some("FIT-1")),
        row("5", "X", Some("FIT-1")),
    ];
    let ids = fingerprints(&rows);
    assert_eq!(ids[0], expected);
    assert_eq!(ids[1], format!("{expected}-2"));
    assert_eq!(ids[2].len(), 15);
    assert_eq!(&ids[3..], ["FIT-1", "FIT-1-2"]);
    assert_eq!(two_decimals("-80.005".parse().unwrap()), "-80.00");
    assert_eq!(two_decimals("2.675".parse().unwrap()), "2.68");
}

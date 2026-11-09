use super::*;

#[test]
fn prefixes_go() {
    assert_eq!(strip_prefixes("SQ *BLUE BOTTLE"), "BLUE BOTTLE");
    assert_eq!(strip_prefixes("pos debit TST* Taqueria"), "Taqueria");
    assert_eq!(strip_prefixes("SPOTIFY"), "SPOTIFY");
    assert_eq!(strip_prefixes("SQ"), "SQ");
}

#[test]
fn numbers_and_dates_are_masked() {
    assert_eq!(
        description("SQ *CORNER CAFE #0412 08/03 CARD 123456789"),
        "CORNER CAFE # # CARD #"
    );
    assert_eq!(description("ACME PAYROLL 2026-08-03"), "ACME PAYROLL #");
    assert_eq!(description("REF1234567X 7-ELEVEN 42"), "REF#X 7-ELEVEN 42");
}

#[test]
fn keys_ignore_numbers_case_and_punctuation() {
    assert_eq!(key("SQ *CORNER CAFE #0412 08/03"), "corner cafe");
    assert_eq!(key("Corner Café 99"), "corner café");
    assert_eq!(key("7-ELEVEN 3321"), "eleven");
    assert_eq!(key("123456"), "");
}

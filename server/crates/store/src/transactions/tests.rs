#[test]
fn wildcards_are_literal() {
    assert_eq!(super::contains_pattern("50%_off\\"), "%50\\%\\_off\\\\%");
}

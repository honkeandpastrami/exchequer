use super::*;
use proptest::prelude::*;

#[test]
fn masked_by_default() {
    let n = AccountNumber::new(" 000123456789 ").unwrap();
    assert_eq!(n.to_string(), "…6789");
    assert_eq!(format!("{n:?}"), "AccountNumber(…6789)");
    assert_eq!(n.reveal(), "000123456789");
    assert_eq!(AccountNumber::new("12").unwrap().to_string(), "…12");
    assert!(AccountNumber::new("  ").is_none());
    assert!(AccountNumber::new(" - ").is_none());
    assert_eq!(
        AccountNumber::new("1234-5678 90"),
        AccountNumber::new("1234567890")
    );
}

proptest! {
    #[test]
    fn formatting_shows_at_most_four_digits(digits in "[0-9]{5,20}") {
        let n = AccountNumber::new(&digits).unwrap();
        for shown in [n.to_string(), format!("{n:?}"), format!("{:?}", Some(&n))] {
            let count = shown.chars().filter(char::is_ascii_digit).count();
            prop_assert!(count <= 4, "{shown}");
        }
    }
}

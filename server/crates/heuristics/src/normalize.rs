//! Bank texts, normalized: the same merchant's rows differ in card and
//! reference numbers, store numbers, dates and the payment processor's
//! prefix; these are stripped so the rows look alike.

/// Payment processors' prefixes (`SQ *COFFEE` is Square's), and banks'
/// words for a card purchase. Matched at the start, ignoring case.
const PREFIXES: &[&str] = &[
    "SQ *",
    "SQ*",
    "TST* ",
    "TST*",
    "SP * ",
    "SP *",
    "PP*",
    "PAYPAL *",
    "PY *",
    "IN *",
    "POS DEBIT ",
    "POS PURCHASE ",
    "POS ",
    "DEBIT CARD PURCHASE ",
    "CHECKCARD ",
    "PURCHASE AUTHORIZED ON ",
    "RECURRING PAYMENT ",
    "DBT CRD ",
];

/// `s` without processor prefixes.
#[must_use]
pub fn strip_prefixes(s: &str) -> &str {
    let mut s = s.trim();
    'again: loop {
        for p in PREFIXES {
            if s.len() >= p.len()
                && s.is_char_boundary(p.len())
                && s[..p.len()].eq_ignore_ascii_case(p)
            {
                s = s[p.len()..].trim_start();
                continue 'again;
            }
        }
        return s;
    }
}

/// A date such as `08/03`, `8-3-26` or `2026-08-03`.
fn date_like(t: &str) -> bool {
    let parts: Vec<&str> = t.split(['/', '-']).collect();
    (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|p| (1..=4).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}

/// For showing and sending (design §8): no processor prefix, and dates,
/// store numbers (`#123`) and runs of 5 or more digits each replaced by
/// `#`, so it can't carry a card or account number.
#[must_use]
pub fn description(s: &str) -> String {
    strip_prefixes(s)
        .split_whitespace()
        .map(|t| {
            if date_like(t)
                || (t.len() > 1 && t.starts_with('#') && t[1..].bytes().all(|b| b.is_ascii_digit()))
            {
                return "#".to_owned();
            }
            let mut out = String::new();
            let mut digits = String::new();
            for c in t.chars().chain(std::iter::once(' ')) {
                if c.is_ascii_digit() {
                    digits.push(c);
                    continue;
                }
                if digits.len() >= 5 {
                    out.push('#');
                } else {
                    out.push_str(&digits);
                }
                digits.clear();
                if c != ' ' {
                    out.push(c);
                }
            }
            out
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// What rows are compared by: the words of the text, lowercase, without
/// digits or punctuation. Rows of the same merchant usually share it.
#[must_use]
pub fn key(s: &str) -> String {
    strip_prefixes(s)
        .chars()
        .map(|c| {
            if c.is_alphabetic() {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
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
}

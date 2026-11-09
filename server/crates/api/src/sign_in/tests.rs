use super::*;

#[test]
fn user_codes_are_read_forgivingly() {
    assert_eq!(user_code("bcdf ghjk").as_deref(), Some("BCDF-GHJK"));
    assert_eq!(user_code("BCDF-GHJK").as_deref(), Some("BCDF-GHJK"));
    assert_eq!(user_code("BCDF-GHJ"), None);
    assert_eq!(user_code("ABCD-EFGH"), None); // vowels never appear
    let fresh = new_user_code().unwrap();
    assert_eq!(user_code(&fresh), Some(fresh));
}

#[test]
fn the_origin_names_the_relying_party() {
    assert!(passkeys("https://books.example.org/exchequer/").is_ok());
    assert!(passkeys("http://localhost:5173").is_ok());
    assert!(passkeys("not a url").is_err());
    assert!(passkeys("http://books.example.org").is_err());
}

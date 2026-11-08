use super::*;

#[test]
fn generated_secrets_parse_back() {
    let t = generate(Kind::Token).unwrap();
    let (id, secret) = parse(Kind::Token, &t.text).unwrap();
    assert_eq!(id, t.id);
    assert_eq!(t.hash, hash(secret));
    assert_eq!(t.hash.len(), 64);
    assert!(!t.hash.contains(secret));
    assert!(check(Kind::Token, &t.text, |i| (i == t.id)
        .then(|| t.hash.clone())));
}

#[test]
fn a_secret_is_only_good_for_its_kind() {
    let invite = generate(Kind::Invite).unwrap();
    assert!(invite.text.starts_with("exi_"));
    assert!(parse(Kind::Token, &invite.text).is_none());
    assert!(!check(Kind::Token, &invite.text, |_| Some(
        invite.hash.clone()
    )));
    assert!(!check(
        Kind::Invite,
        &generate(Kind::Invite).unwrap().text,
        |_| Some(invite.hash.clone())
    ));
}

#[test]
fn malformed_tokens() {
    let good = generate(Kind::Token).unwrap().text;
    for bad in [
        "",
        "exq_",
        &good[4..],
        &good[..good.len() - 1],
        &good.replace("exq_", "xyz_"),
        &format!("{good}0"),
        "exq_0123456789abcdef0123456789abcdef_zz",
    ] {
        assert!(parse(Kind::Token, bad).is_none(), "{bad}");
    }
}

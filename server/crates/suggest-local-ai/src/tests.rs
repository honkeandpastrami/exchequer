#[test]
fn methods_are_found_by_id() {
    assert_eq!(
        super::method("lexical-knn-v2").unwrap().name(),
        "lexical-knn"
    );
    assert!(super::method("lexical-knn-v1").is_none());
    assert_eq!(super::METHODS[0].id(), "lexical-knn-v2");
}

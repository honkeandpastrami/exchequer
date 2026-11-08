use super::*;

#[test]
fn new_ids_sort_by_creation() {
    let a = AccountId::new();
    let b = AccountId::new();
    assert!(a < b);
}

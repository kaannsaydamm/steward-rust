use super::entries;
use std::collections::HashSet;

#[test]
fn catalog_is_non_empty() {
    assert!(!entries().is_empty());
}

#[test]
fn catalog_ids_are_unique() {
    let mut seen = HashSet::new();
    for entry in entries() {
        assert!(
            seen.insert(entry.catalog_id),
            "duplicate catalog id: {}",
            entry.catalog_id
        );
    }
}

#[test]
fn every_entry_has_a_runnable_command() {
    for entry in entries() {
        assert!(!entry.command.trim().is_empty());
        assert!(!entry.name.trim().is_empty());
        assert!(!entry.description.trim().is_empty());
    }
}

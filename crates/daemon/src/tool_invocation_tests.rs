use super::truncate_utf8;

#[test]
fn audit_truncation_preserves_utf8_boundaries() {
    let value = "abcğdef";

    assert_eq!(truncate_utf8(value, 4), "abc");
    assert_eq!(truncate_utf8(value, 5), "abcğ");
    assert_eq!(truncate_utf8(value, 99), value);
}

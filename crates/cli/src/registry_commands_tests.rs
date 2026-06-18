use super::parse_arguments;

#[test]
fn parser_preserves_equals_inside_argument_value() {
    let arguments = parse_arguments(&["query=key=value".to_owned()]).expect("parse arguments");

    assert_eq!(
        arguments.get("query").map(String::as_str),
        Some("key=value")
    );
}

#[test]
fn parser_rejects_duplicate_argument_keys() {
    let result = parse_arguments(&["query=first".to_owned(), "query=second".to_owned()]);

    assert!(result.is_err());
}

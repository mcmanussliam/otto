use otto_cli::envfile::{load, parse};
use tempfile::tempdir;

#[test]
fn parse_envfile() {
    let text = r#"
# comment
FOO=bar
EMPTY=
export NAME=otto
SINGLE='hello world'
DOUBLE="a\\nb"
RAW=hello # trailing comment
"#;

    let out = parse(text).expect("parse dotenv");
    assert_eq!(out.get("FOO"), Some(&"bar".to_string()));
    assert_eq!(out.get("EMPTY"), Some(&"".to_string()));
    assert_eq!(out.get("NAME"), Some(&"otto".to_string()));
    assert_eq!(out.get("SINGLE"), Some(&"hello world".to_string()));
    assert_eq!(out.get("DOUBLE"), Some(&"a\\nb".to_string()));
    assert_eq!(out.get("RAW"), Some(&"hello".to_string()));
}

#[test]
fn parse_rejects_invalid_line() {
    assert!(parse("not-valid").is_err());
}

#[test]
fn load_missing_file() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("missing.env");
    let err = load(&path).expect_err("expected missing");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

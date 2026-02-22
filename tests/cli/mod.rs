use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

#[test]
fn run_inline_json_preserves_unknown_variable_token() {
    let dir = tempdir().expect("tempdir");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("otto");
    let out = cmd
        .current_dir(dir.path())
        .args(["run", "--json", "--", "echo", "${NOPE}"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let parsed: Value = serde_json::from_slice(&out).expect("run json");
    assert_eq!(parsed["command_preview"], "echo ${NOPE}");
}

#[test]
fn tasks_output_compacts_command_whitespace() {
    let dir = tempdir().expect("tempdir");
    fs::write(
        dir.path().join("otto.yml"),
        r#"version: 1

defaults:
  notify_on: never

notifications:
  desktop: false

tasks:
  demo:
    run: |
      echo one
      two   three
"#,
    )
    .expect("write config");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("otto");
    cmd.current_dir(dir.path())
        .args(["tasks"])
        .assert()
        .success()
        .stdout(predicate::str::contains("echo one two three"));
}

#[test]
fn validate_json_reports_valid_config() {
    let dir = tempdir().expect("tempdir");
    fs::write(
        dir.path().join("otto.yml"),
        r#"version: 1

tasks:
  test:
    exec: ["echo", "ok"]
"#,
    )
    .expect("write config");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("otto");
    let out = cmd
        .current_dir(dir.path())
        .args(["validate", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let parsed: Value = serde_json::from_slice(&out).expect("validate json");
    assert_eq!(parsed["valid"], true);
    assert_eq!(parsed["config"], "./otto.yml");
    assert!(parsed.get("issues").is_none());
    assert!(parsed.get("error").is_none());
}

#[test]
fn validate_json_reports_invalid_config_and_fails() {
    let dir = tempdir().expect("tempdir");
    fs::write(
        dir.path().join("otto.yml"),
        r#"version: 1

tasks:
  bad:
    description: broken
"#,
    )
    .expect("write config");

    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("otto");
    let out = cmd
        .current_dir(dir.path())
        .args(["validate", "--json"])
        .assert()
        .failure()
        .code(2)
        .get_output()
        .stdout
        .clone();

    let parsed: Value = serde_json::from_slice(&out).expect("validate json");
    assert_eq!(parsed["valid"], false);
    assert_eq!(parsed["config"], "./otto.yml");
    assert!(parsed["issues"].is_array());
    assert!(parsed["issues"][0]["field"].as_str().is_some());
    assert!(parsed["issues"][0]["message"].as_str().is_some());
    assert!(parsed["error"].as_str().is_some());
}

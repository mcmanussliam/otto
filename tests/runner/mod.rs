use otto_cli::model::RunStatus;
use otto_cli::runner::{Request, execute, tail};
use std::collections::HashMap;
use std::time::Duration;
use tempfile::tempdir;

fn base_request() -> Request {
    Request {
        name: "inline".to_string(),
        command_preview: "echo ok".to_string(),
        use_shell: false,
        exec: vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "echo ok".to_string(),
        ],
        shell: String::new(),
        dir: String::new(),
        env: HashMap::new(),
        timeout: Duration::ZERO,
        retries: 0,
        retry_backoff: Duration::from_millis(10),
        stream_output: false,
    }
}

#[test]
fn execute_success() {
    let result = execute(&base_request()).expect("success");
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.status, RunStatus::Success);
}

#[test]
fn execute_failure() {
    let mut req = base_request();
    req.exec = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "echo err >&2; exit 7".to_string(),
    ];

    let err = execute(&req).expect_err("expected failure");
    assert_eq!(err.result.exit_code, 7);
    assert_eq!(err.result.status, RunStatus::Failed);
}

#[test]
fn execute_timeout() {
    let mut req = base_request();
    req.exec = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "sleep 1".to_string(),
    ];
    req.timeout = Duration::from_millis(50);

    let err = execute(&req).expect_err("expected timeout");
    assert_eq!(err.result.exit_code, 124);
}

#[test]
fn execute_retry_then_success() {
    let dir = tempdir().expect("tempdir");
    let flag = dir.path().join("flag");
    let script = format!(
        r#"[ -f "{}" ] || {{ touch "{}"; exit 9; }}; exit 0"#,
        flag.display(),
        flag.display()
    );

    let mut req = base_request();
    req.use_shell = true;
    req.exec.clear();
    req.shell = script;
    req.retries = 1;

    let result = execute(&req).expect("retry succeeds");
    assert_eq!(result.exit_code, 0);
}

#[test]
fn validate_request_retries() {
    let mut req = base_request();
    req.retries = -1;
    assert!(execute(&req).is_err());
}

#[test]
fn tail_limits_output() {
    let input = "a\nb\nc\nd\ne\nf";
    let out = tail(input, 3, 10).expect("tail");
    assert_eq!(out, "d\ne\nf");
}

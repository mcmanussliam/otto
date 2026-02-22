use otto_cli::config::{
    self, Config, Defaults, Notifications, Task, load, resolve_inline, validate,
};
use std::collections::HashMap;
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn validate_rejects_task_with_exec_and_run() {
    let mut tasks = HashMap::new();
    tasks.insert(
        "build".to_string(),
        Task {
            exec: vec!["cargo".to_string(), "build".to_string()],
            run: "cargo build".to_string(),
            ..Task::default()
        },
    );

    let cfg = Config {
        version: 1,
        tasks: Some(tasks),
        ..Config::default()
    };

    assert!(validate(&cfg).is_err());
}

#[test]
fn resolve_task_applies_defaults() {
    let mut tasks = HashMap::new();
    tasks.insert(
        "test".to_string(),
        Task {
            exec: vec!["cargo".to_string(), "test".to_string()],
            ..Task::default()
        },
    );

    let cfg = Config {
        version: 1,
        defaults: Defaults {
            timeout: "3s".to_string(),
            retries: Some(2),
            retry_backoff: "2s".to_string(),
            notify_on: "always".to_string(),
        },
        tasks: Some(tasks),
        ..Config::default()
    };

    let resolved = cfg.resolve_task("test").expect("resolve task");
    assert_eq!(resolved.timeout, Duration::from_secs(3));
    assert_eq!(resolved.retries, 2);
    assert_eq!(resolved.retry_backoff, Duration::from_secs(2));
    assert_eq!(resolved.notify_on, "always");
}

#[test]
fn resolve_inline_uses_defaults_and_overrides() {
    let defaults = Defaults {
        timeout: "4s".to_string(),
        retries: Some(3),
        retry_backoff: "2s".to_string(),
        notify_on: "always".to_string(),
    };

    let args = vec!["cargo".to_string(), "test".to_string()];
    let resolved = resolve_inline(&args, "", "", None, "", &defaults).expect("resolve inline");
    assert_eq!(resolved.name, "inline");
    assert_eq!(resolved.timeout, Duration::from_secs(4));
    assert_eq!(resolved.retries, 3);
    assert_eq!(resolved.notify_on, "always");

    let override_args = vec!["echo".to_string(), "ok".to_string()];
    let overridden = resolve_inline(&override_args, "quick", "1s", Some(1), "failure", &defaults)
        .expect("resolve inline override");
    assert_eq!(overridden.name, "quick");
    assert_eq!(overridden.timeout, Duration::from_secs(1));
    assert_eq!(overridden.retries, 1);
    assert_eq!(overridden.notify_on, "failure");
}

#[test]
fn load_rejects_unknown_field() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("otto.yml");

    fs::write(
        &path,
        r#"version: 1
tasks:
  test:
    exec: ["echo", "ok"]
    unexpected: true
"#,
    )
    .expect("write config");

    assert!(load(&path).is_err());
}

#[test]
fn resolve_notification_settings_defaults_and_override() {
    let cfg = Config::default();
    let settings = cfg
        .resolve_notification_settings()
        .expect("default settings");
    assert!(settings.desktop_enabled);
    assert_eq!(settings.webhook_timeout, Duration::from_secs(5));

    let cfg = Config {
        notifications: Notifications {
            desktop: Some(false),
            webhook_url: "https://example.com".to_string(),
            webhook_timeout: "2s".to_string(),
        },
        ..Config::default()
    };

    let settings = cfg
        .resolve_notification_settings()
        .expect("override settings");
    assert!(!settings.desktop_enabled);
    assert_eq!(settings.webhook_timeout, Duration::from_secs(2));
}

#[test]
fn resolve_inline_rejects_invalid_retries() {
    let args = vec!["echo".to_string(), "ok".to_string()];
    let err = resolve_inline(&args, "", "", Some(11), "", &Defaults::default())
        .expect_err("expected invalid retries");
    assert!(err.contains("between 0 and 10"));
}

#[test]
fn resolve_task_supports_composed_tasks() {
    let mut tasks = HashMap::new();
    tasks.insert(
        "ci".to_string(),
        Task {
            tasks: vec![
                "lint".to_string(),
                "build".to_string(),
                "clippy".to_string(),
            ],
            parallel: true,
            ..Task::default()
        },
    );
    tasks.insert(
        "lint".to_string(),
        Task {
            exec: vec![
                "cargo".to_string(),
                "fmt".to_string(),
                "--check".to_string(),
            ],
            ..Task::default()
        },
    );
    tasks.insert(
        "build".to_string(),
        Task {
            exec: vec!["cargo".to_string(), "build".to_string()],
            ..Task::default()
        },
    );
    tasks.insert(
        "clippy".to_string(),
        Task {
            exec: vec!["cargo".to_string(), "clippy".to_string()],
            ..Task::default()
        },
    );

    let cfg = Config {
        version: 1,
        tasks: Some(tasks),
        ..Config::default()
    };

    let resolved = cfg.resolve_task("ci").expect("resolve composed task");
    assert_eq!(resolved.sub_tasks.len(), 3);
    assert!(resolved.parallel);
    assert!(!resolved.command_preview.is_empty());
}

#[test]
fn validate_rejects_unknown_composed_task_reference() {
    let mut tasks = HashMap::new();
    tasks.insert(
        "ci".to_string(),
        Task {
            tasks: vec!["lint".to_string(), "missing".to_string()],
            ..Task::default()
        },
    );
    tasks.insert(
        "lint".to_string(),
        Task {
            exec: vec![
                "cargo".to_string(),
                "fmt".to_string(),
                "--check".to_string(),
            ],
            ..Task::default()
        },
    );

    let cfg = Config {
        version: 1,
        tasks: Some(tasks),
        ..Config::default()
    };

    let err = validate(&cfg).expect_err("expected validation error");
    assert!(err.to_string().contains("unknown task"));
}

#[test]
fn validate_rejects_composed_task_with_exec_or_run() {
    let mut tasks = HashMap::new();
    tasks.insert(
        "ci".to_string(),
        Task {
            tasks: vec!["lint".to_string()],
            run: "cargo test".to_string(),
            ..Task::default()
        },
    );

    let cfg = Config {
        version: config::CURRENT_VERSION,
        tasks: Some(tasks),
        ..Config::default()
    };

    assert!(validate(&cfg).is_err());
}

#[test]
fn validate_rejects_reserved_task_name_validate() {
    let mut tasks = HashMap::new();
    tasks.insert(
        "validate".to_string(),
        Task {
            exec: vec!["echo".to_string(), "ok".to_string()],
            ..Task::default()
        },
    );

    let cfg = Config {
        version: config::CURRENT_VERSION,
        tasks: Some(tasks),
        ..Config::default()
    };

    let err = validate(&cfg).expect_err("expected reserved task-name error");
    assert!(err.to_string().contains("name is reserved"));
}

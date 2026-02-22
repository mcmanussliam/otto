use otto_cli::notify::{Event, Manager};
use std::time::Duration;
use time::OffsetDateTime;

fn test_event() -> Event {
    Event {
        name: "inline".to_string(),
        source: "inline".to_string(),
        status: "success".to_string(),
        exit_code: 0,
        duration: Duration::from_millis(500),
        started_at: OffsetDateTime::now_utc(),
        command_preview: "echo ok".to_string(),
        stderr_tail: None,
    }
}

#[test]
fn notify_webhook_failure_request() {
    let manager = Manager {
        desktop_enabled: false,
        webhook_url: "http://127.0.0.1:1/webhook".to_string(),
        webhook_timeout: Duration::from_secs(1),
    };

    let err = manager.notify(&test_event()).expect_err("expected failure");
    assert!(err.contains("webhook:"));
}

#[test]
fn notify_no_providers() {
    let manager = Manager {
        desktop_enabled: false,
        webhook_url: String::new(),
        webhook_timeout: Duration::from_secs(1),
    };

    manager.notify(&test_event()).expect("no-provider notify");
}

use otto_cli::history::{Filter, Store};
use otto_cli::model::{RunRecord, RunSource, RunStatus};
use std::fs::OpenOptions;
use std::io::Write;
use tempfile::tempdir;
use time::OffsetDateTime;

fn record(id: &str, source: RunSource, status: RunStatus) -> RunRecord {
    RunRecord {
        id: id.to_string(),
        name: id.to_string(),
        source,
        command_preview: "echo ok".to_string(),
        started_at: OffsetDateTime::now_utc(),
        duration_ms: 10,
        exit_code: if status == RunStatus::Success { 0 } else { 1 },
        status,
        stderr_tail: None,
    }
}

#[test]
fn append_and_list() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("history.jsonl");
    let store = Store::new(&path);

    store
        .append(&record("1", RunSource::Task, RunStatus::Success))
        .expect("append first");
    store
        .append(&record("2", RunSource::Inline, RunStatus::Failed))
        .expect("append second");

    let rows = store
        .list(&Filter {
            limit: Some(10),
            ..Filter::default()
        })
        .expect("list");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, "2");

    let filtered = store
        .list(&Filter {
            status: Some("failed".to_string()),
            ..Filter::default()
        })
        .expect("filter");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "2");
}

#[test]
fn list_ignores_malformed_lines() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("history.jsonl");
    let store = Store::new(&path);

    store
        .append(&record("good", RunSource::Task, RunStatus::Success))
        .expect("append");

    let mut file = OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open for append");
    writeln!(file, "{{bad").expect("write malformed");

    let rows = store.list(&Filter::default()).expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "good");
}

#[test]
fn list_missing_file() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("missing.jsonl");
    let store = Store::new(path);
    let rows = store.list(&Filter::default()).expect("list");
    assert!(rows.is_empty());
}

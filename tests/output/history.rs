use otto_cli::model::{RunSource, RunStatus};
use otto_cli::output::{HistoryRow, print_history};
use time::OffsetDateTime;

#[test]
fn print_history_empty() {
    let mut out = Vec::new();
    print_history(&mut out, &[]).expect("print history");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("No run history yet"));
}

#[test]
fn print_history_rows() {
    let mut out = Vec::new();
    let rows = vec![HistoryRow {
        name: "inline".to_string(),
        source: RunSource::Inline,
        status: RunStatus::Success,
        exit_code: 0,
        started_at: OffsetDateTime::now_utc(),
        duration_ms: 25,
    }];
    print_history(&mut out, &rows).expect("print history");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("inline"));
    assert!(text.contains("success"));
    assert!(text.contains("source: inline"));
}

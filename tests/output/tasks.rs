use otto_cli::output::{TaskRow, print_tasks};

#[test]
fn print_tasks_empty() {
    let mut out = Vec::new();
    print_tasks(&mut out, &[]).expect("print tasks");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("No tasks found"));
}

#[test]
fn print_tasks_rows() {
    let mut out = Vec::new();
    let rows = vec![TaskRow {
        name: "test".to_string(),
        description: "run tests".to_string(),
        command: "cargo test".to_string(),
    }];
    print_tasks(&mut out, &rows).expect("print tasks");
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.contains("test"));
    assert!(text.contains("command:"));
}

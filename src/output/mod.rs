mod history;
mod style;
mod tasks;

pub use history::{HistoryRow, print_history};
pub use style::{
    accent, bold, bullet, command, configure, failure, info, muted, number, success, warning,
};
pub use tasks::{TaskRow, print_tasks};

pub fn format_duration_ms(ms: i64) -> String {
    if ms < 1000 {
        return format!("{ms}ms");
    }

    if ms.rem_euclid(1000) == 0 {
        return format!("{}s", ms / 1000);
    }

    format!("{:.3}s", ms as f64 / 1000.0)
}

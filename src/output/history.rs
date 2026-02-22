use crate::model::{RunSource, RunStatus};
use crate::output::{accent, failure, format_duration_ms, info, number, success};
use std::io::Write;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct HistoryRow {
    pub name: String,
    pub source: RunSource,
    pub status: RunStatus,
    pub exit_code: i32,
    pub started_at: OffsetDateTime,
    pub duration_ms: i64,
}

pub fn print_history(mut w: impl Write, rows: &[HistoryRow]) -> std::io::Result<()> {
    if rows.is_empty() {
        writeln!(w, "{} No run history yet.", info("i"))?;
        return Ok(());
    }

    for (idx, row) in rows.iter().enumerate() {
        let source = match row.source {
            RunSource::Task => "task",
            RunSource::Inline => "inline",
        };

        let status = match row.status {
            RunStatus::Success => success("ok success"),
            RunStatus::Failed => failure("x failed"),
        };

        let started = row
            .started_at
            .format(&time::macros::format_description!(
                "[year]-[month]-[day] [hour]:[minute]:[second]"
            ))
            .unwrap_or_else(|_| "-".to_string());

        writeln!(w, "{} {}", accent(&row.name), status)?;
        writeln!(w, "  source: {}", source)?;
        writeln!(w, "  exit: {}", number(&row.exit_code.to_string()))?;
        writeln!(w, "  started (UTC): {}", started)?;
        writeln!(
            w,
            "  duration: {}",
            number(&format_duration_ms(row.duration_ms))
        )?;

        if idx + 1 < rows.len() {
            writeln!(w)?;
        }
    }

    Ok(())
}

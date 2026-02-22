use crate::output::{bold, command, info, muted};
use std::io::Write;

#[derive(Debug, Clone)]
pub struct TaskRow {
    pub name: String,
    pub description: String,
    pub command: String,
}

pub fn print_tasks(mut w: impl Write, rows: &[TaskRow]) -> std::io::Result<()> {
    if rows.is_empty() {
        writeln!(w, "{} {}", info("i"), muted("No tasks found."))?;
        return Ok(());
    }

    let mut normalized = Vec::with_capacity(rows.len());

    for row in rows {
        let description = if row.description.trim().is_empty() {
            String::new()
        } else {
            row.description.clone()
        };

        let command = if row.command.trim().is_empty() {
            "-".to_string()
        } else {
            row.command.clone()
        };

        normalized.push(TaskRow {
            name: row.name.clone(),
            description,
            command,
        });
    }

    for (idx, row) in normalized.iter().enumerate() {
        writeln!(w, "{}", bold(&row.name))?;

        if !row.description.is_empty() {
            writeln!(w, "  description: {}", &row.description)?;
        }

        writeln!(w, "  command: {}", command(&row.command),)?;

        if idx + 1 < normalized.len() {
            writeln!(w)?;
        }
    }

    Ok(())
}

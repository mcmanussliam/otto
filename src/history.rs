use crate::model::RunRecord;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_PATH: &str = ".otto/history.jsonl";

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub limit: Option<usize>,
    pub status: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, record: &RunRecord) -> Result<(), String> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "invalid history path".to_string())?;

        fs::create_dir_all(parent).map_err(|e| format!("create history directory: {e}"))?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("open history file: {e}"))?;

        let line =
            serde_json::to_vec(record).map_err(|e| format!("serialize history record: {e}"))?;
        file.write_all(&line)
            .and_then(|_| file.write_all(b"\n"))
            .map_err(|e| format!("write history record: {e}"))
    }

    pub fn list(&self, filter: &Filter) -> Result<Vec<RunRecord>, String> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(format!("open history file: {err}")),
        };

        let mut records = Vec::new();
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let Ok(line) = line else {
                continue;
            };
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let Ok(rec) = serde_json::from_str::<RunRecord>(trimmed) else {
                continue;
            };

            if !matches_filter(&rec, filter) {
                continue;
            }

            records.push(rec);
        }

        records.reverse();

        if let Some(limit) = filter.limit
            && records.len() > limit
        {
            records.truncate(limit);
        }

        Ok(records)
    }
}

fn matches_filter(record: &RunRecord, filter: &Filter) -> bool {
    if let Some(status) = &filter.status {
        let current = match record.status {
            crate::model::RunStatus::Success => "success",
            crate::model::RunStatus::Failed => "failed",
        };
        if current != status {
            return false;
        }
    }

    if let Some(source) = &filter.source {
        let current = match record.source {
            crate::model::RunSource::Task => "task",
            crate::model::RunSource::Inline => "inline",
        };
        if current != source {
            return false;
        }
    }

    true
}

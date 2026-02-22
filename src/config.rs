use crate::model::RunSource;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

pub const CURRENT_VERSION: i32 = 1;

static TASK_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9_-]{0,62}$").expect("valid regex"));

const RESERVED_NAMES: &[&str] = &[
    "init",
    "run",
    "history",
    "tasks",
    "validate",
    "version",
    "completion",
];
const VALID_NOTIFY_ON: &[&str] = &["never", "failure", "always"];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub version: i32,
    pub defaults: Defaults,
    pub notifications: Notifications,
    pub tasks: Option<HashMap<String, Task>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Defaults {
    pub timeout: String,
    pub retries: Option<i32>,
    pub retry_backoff: String,
    pub notify_on: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Notifications {
    pub desktop: Option<bool>,
    pub webhook_url: String,
    pub webhook_timeout: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Task {
    pub description: String,
    pub exec: Vec<String>,
    pub run: String,
    pub tasks: Vec<String>,
    pub parallel: bool,
    pub dir: String,
    pub env: HashMap<String, String>,
    pub timeout: String,
    pub retries: Option<i32>,
    pub retry_backoff: String,
    pub notify_on: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedTask {
    pub name: String,
    pub source: RunSource,
    pub command_preview: String,
    pub sub_tasks: Vec<String>,
    pub parallel: bool,
    pub use_shell: bool,
    pub exec: Vec<String>,
    pub shell: String,
    pub dir: String,
    pub env: HashMap<String, String>,
    pub timeout: Duration,
    pub retries: i32,
    pub retry_backoff: Duration,
    pub notify_on: String,
}

#[derive(Debug, Clone)]
pub struct NotificationSettings {
    pub desktop_enabled: bool,
    pub webhook_url: String,
    pub webhook_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ValidationErrors {
    pub issues: Vec<ValidationError>,
}

impl ValidationErrors {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<F: Into<String>, M: Into<String>>(&mut self, field: F, message: M) {
        self.issues.push(ValidationError {
            field: field.into(),
            message: message.into(),
        });
    }

    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty()
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(first) = self.issues.first() {
            write!(
                f,
                "configuration validation failed: {}: {}",
                first.field, first.message
            )
        } else {
            write!(f, "configuration validation failed")
        }
    }
}

impl std::error::Error for ValidationErrors {}

pub fn load(path: &Path) -> Result<Config, String> {
    let cfg = parse(path)?;
    validate(&cfg).map_err(|e| e.to_string())?;
    Ok(cfg)
}

pub fn parse(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read config: {e}"))?;
    let cfg: Config = serde_yaml::from_str(&text).map_err(|e| format!("parse config yaml: {e}"))?;
    Ok(cfg)
}

pub fn validate(cfg: &Config) -> Result<(), ValidationErrors> {
    let mut issues = ValidationErrors::new();

    if cfg.version != CURRENT_VERSION {
        issues.add("version", format!("must be {CURRENT_VERSION}"));
    }

    validate_defaults(&mut issues, &cfg.defaults);
    validate_notifications(&mut issues, &cfg.notifications);

    match &cfg.tasks {
        None => issues.add("tasks", "is required"),
        Some(tasks) => {
            if tasks.is_empty() {
                issues.add("tasks", "is required");
            }
            for (name, task) in tasks {
                validate_task_name(&mut issues, name);
                validate_task(&mut issues, name, task);
            }
            validate_task_dependencies(&mut issues, tasks);
        }
    }

    if issues.has_issues() {
        Err(issues)
    } else {
        Ok(())
    }
}

impl Config {
    pub fn resolve_task(&self, name: &str) -> Result<ResolvedTask, String> {
        let tasks = self
            .tasks
            .as_ref()
            .ok_or_else(|| "tasks: is required".to_string())?;

        let task = tasks
            .get(name)
            .ok_or_else(|| format!("task {name:?} not found"))?;

        let timeout = resolve_duration(&task.timeout, &self.defaults.timeout, Duration::ZERO)
            .map_err(|e| format!("task {name:?} timeout: {e}"))?;
        let retries = resolve_retries(task.retries, self.defaults.retries, 0);
        let retry_backoff = resolve_duration(
            &task.retry_backoff,
            &self.defaults.retry_backoff,
            Duration::from_secs(1),
        )
        .map_err(|e| format!("task {name:?} retry_backoff: {e}"))?;
        let notify_on = resolve_notify_on(&task.notify_on, &self.defaults.notify_on, "failure");

        let mut resolved = ResolvedTask {
            name: name.to_string(),
            source: RunSource::Task,
            command_preview: String::new(),
            sub_tasks: Vec::new(),
            parallel: task.parallel,
            use_shell: false,
            exec: Vec::new(),
            shell: String::new(),
            dir: task.dir.clone(),
            env: task.env.clone(),
            timeout,
            retries,
            retry_backoff,
            notify_on,
        };

        if !task.exec.is_empty() {
            resolved.use_shell = false;
            resolved.exec = task.exec.clone();
            resolved.command_preview = join_command_preview(&task.exec);
        } else if !task.tasks.is_empty() {
            resolved.sub_tasks = task.tasks.clone();
            resolved.command_preview = join_task_preview(&task.tasks, task.parallel);
        } else {
            resolved.use_shell = true;
            resolved.shell = task.run.clone();
            resolved.command_preview = task.run.clone();
        }

        Ok(resolved)
    }

    pub fn resolve_notification_settings(&self) -> Result<NotificationSettings, String> {
        let desktop_enabled = self.notifications.desktop.unwrap_or(true);
        let webhook_timeout = resolve_duration(
            &self.notifications.webhook_timeout,
            "",
            Duration::from_secs(5),
        )
        .map_err(|e| format!("notifications.webhook_timeout: {e}"))?;

        Ok(NotificationSettings {
            desktop_enabled,
            webhook_url: self.notifications.webhook_url.clone(),
            webhook_timeout,
        })
    }
}

pub fn resolve_inline(
    args: &[String],
    name: &str,
    timeout_flag: &str,
    retries_flag: Option<i32>,
    notify_on_flag: &str,
    defaults: &Defaults,
) -> Result<ResolvedTask, String> {
    if args.is_empty() {
        return Err("inline command is required after --".to_string());
    }

    let timeout = resolve_duration(timeout_flag, &defaults.timeout, Duration::ZERO)
        .map_err(|e| format!("inline timeout: {e}"))?;

    let retries = match retries_flag {
        Some(v) => v,
        None => resolve_retries(None, defaults.retries, 0),
    };

    if !(0..=10).contains(&retries) {
        return Err("inline retries must be between 0 and 10".to_string());
    }

    let retry_backoff = resolve_duration("", &defaults.retry_backoff, Duration::from_secs(1))
        .map_err(|e| format!("inline retry_backoff: {e}"))?;

    let notify_on = resolve_notify_on(notify_on_flag, &defaults.notify_on, "failure");
    let task_name = if name.trim().is_empty() {
        "inline".to_string()
    } else {
        name.to_string()
    };

    Ok(ResolvedTask {
        name: task_name,
        source: RunSource::Inline,
        command_preview: join_command_preview(args),
        sub_tasks: Vec::new(),
        parallel: false,
        use_shell: false,
        exec: args.to_vec(),
        shell: String::new(),
        dir: String::new(),
        env: HashMap::new(),
        timeout,
        retries,
        retry_backoff,
        notify_on,
    })
}

fn validate_defaults(issues: &mut ValidationErrors, d: &Defaults) {
    if !d.timeout.is_empty() && parse_duration(&d.timeout).is_err() {
        issues.add("defaults.timeout", "must be a valid duration");
    }

    if let Some(retries) = d.retries
        && !(0..=10).contains(&retries)
    {
        issues.add("defaults.retries", "must be between 0 and 10");
    }

    if !d.retry_backoff.is_empty() && parse_duration(&d.retry_backoff).is_err() {
        issues.add("defaults.retry_backoff", "must be a valid duration");
    }

    if !d.notify_on.is_empty() && !VALID_NOTIFY_ON.contains(&d.notify_on.as_str()) {
        issues.add(
            "defaults.notify_on",
            "must be one of never, failure, always",
        );
    }
}

fn validate_notifications(issues: &mut ValidationErrors, n: &Notifications) {
    if !n.webhook_url.is_empty() && reqwest::Url::parse(&n.webhook_url).is_err() {
        issues.add("notifications.webhook_url", "must be a valid URL");
    }

    if !n.webhook_timeout.is_empty() && parse_duration(&n.webhook_timeout).is_err() {
        issues.add("notifications.webhook_timeout", "must be a valid duration");
    }
}

fn validate_task_name(issues: &mut ValidationErrors, name: &str) {
    if !TASK_NAME_RE.is_match(name) {
        issues.add(
            format!("tasks.{name}"),
            "name must match ^[a-z0-9][a-z0-9_-]{0,62}$",
        );
    }

    if RESERVED_NAMES.contains(&name) {
        issues.add(format!("tasks.{name}"), "name is reserved");
    }
}

fn validate_task(issues: &mut ValidationErrors, name: &str, task: &Task) {
    let field = format!("tasks.{name}");
    let has_exec = !task.exec.is_empty();
    let has_run = !task.run.is_empty();
    let has_tasks = !task.tasks.is_empty();
    let mode_count = [has_exec, has_run, has_tasks]
        .into_iter()
        .filter(|mode| *mode)
        .count();

    if mode_count != 1 {
        issues.add(
            field.clone(),
            "must define exactly one of exec, run, or tasks",
        );
    }

    if has_exec {
        for (idx, tok) in task.exec.iter().enumerate() {
            if tok.is_empty() {
                issues.add(format!("{field}.exec[{idx}]"), "must not be empty");
            }
        }
    }

    if !task.timeout.is_empty() && parse_duration(&task.timeout).is_err() {
        issues.add(format!("{field}.timeout"), "must be a valid duration");
    }

    if let Some(retries) = task.retries
        && !(0..=10).contains(&retries)
    {
        issues.add(format!("{field}.retries"), "must be between 0 and 10");
    }

    if !task.retry_backoff.is_empty() && parse_duration(&task.retry_backoff).is_err() {
        issues.add(format!("{field}.retry_backoff"), "must be a valid duration");
    }

    if !task.notify_on.is_empty() && !VALID_NOTIFY_ON.contains(&task.notify_on.as_str()) {
        issues.add(
            format!("{field}.notify_on"),
            "must be one of never, failure, always",
        );
    }

    if has_tasks {
        if !task.dir.is_empty() {
            issues.add(
                format!("{field}.dir"),
                "is not supported when using task composition",
            );
        }
        if !task.env.is_empty() {
            issues.add(
                format!("{field}.env"),
                "is not supported when using task composition",
            );
        }
        if !task.timeout.is_empty() {
            issues.add(
                format!("{field}.timeout"),
                "is not supported when using task composition",
            );
        }
        if task.retries.is_some() {
            issues.add(
                format!("{field}.retries"),
                "is not supported when using task composition",
            );
        }
        if !task.retry_backoff.is_empty() {
            issues.add(
                format!("{field}.retry_backoff"),
                "is not supported when using task composition",
            );
        }
        for (idx, dep) in task.tasks.iter().enumerate() {
            if dep.trim().is_empty() {
                issues.add(format!("{field}.tasks[{idx}]"), "must not be empty");
            }
        }
    }
}

fn parse_duration(text: &str) -> Result<Duration, humantime::DurationError> {
    humantime::parse_duration(text)
}

fn validate_task_dependencies(issues: &mut ValidationErrors, tasks: &HashMap<String, Task>) {
    for (name, task) in tasks {
        if task.tasks.is_empty() {
            continue;
        }

        let field = format!("tasks.{name}.tasks");
        for (idx, dep) in task.tasks.iter().enumerate() {
            if dep == name {
                issues.add(
                    format!("{field}[{idx}]"),
                    "must not reference itself directly",
                );
                continue;
            }

            if !tasks.contains_key(dep) {
                issues.add(
                    format!("{field}[{idx}]"),
                    format!("references unknown task {dep:?}"),
                );
            }
        }
    }
}

fn resolve_duration(
    primary: &str,
    fallback: &str,
    default_value: Duration,
) -> Result<Duration, String> {
    let value = if !primary.is_empty() {
        primary
    } else if !fallback.is_empty() {
        fallback
    } else {
        return Ok(default_value);
    };

    parse_duration(value).map_err(|_| "must be a valid duration".to_string())
}

fn resolve_retries(primary: Option<i32>, fallback: Option<i32>, default_value: i32) -> i32 {
    primary.or(fallback).unwrap_or(default_value)
}

fn resolve_notify_on(primary: &str, fallback: &str, default_value: &str) -> String {
    if !primary.is_empty() {
        primary.to_string()
    } else if !fallback.is_empty() {
        fallback.to_string()
    } else {
        default_value.to_string()
    }
}

fn join_command_preview(args: &[String]) -> String {
    args.join(" ")
}

fn join_task_preview(tasks: &[String], parallel: bool) -> String {
    let mode = if parallel { "parallel" } else { "sequential" };
    format!("tasks ({mode}): {}", tasks.join(", "))
}

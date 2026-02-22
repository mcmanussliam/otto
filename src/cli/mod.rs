use crate::app_error::AppError;
use crate::config::{self, Config, Defaults, NotificationSettings, ResolvedTask};
use crate::history::{DEFAULT_PATH, Filter, Store};
use crate::model::{RunRecord, RunSource, RunStatus};
use crate::notify;
use crate::output::{self, HistoryRow, TaskRow};
use crate::runner::{self, Request};
use crate::version;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Generator, generate};
use rand::Rng;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::time::Instant;
use time::OffsetDateTime;

const DEFAULT_CONFIG_PATH: &str = "./otto.yml";

const DEFAULT_CONFIG_TEMPLATE: &str = r#"version: 1

defaults:
  timeout: "2m"      # max runtime per attempt
  retries: 0          # retries after first failure
  retry_backoff: "1s"
  notify_on: failure  # never | failure | always

notifications:
  desktop: true       # desktop notifications (macOS/Linux)
  # webhook_url: "https://example.com/otto-hook"
  # webhook_timeout: "5s"

tasks:
  test:
    description: run unit tests
    exec: ["cargo", "test"]

  clippy:
    description: run clippy
    exec: ["cargo", "clippy", "--all-targets", "--all-features", "--", "-D", "warnings"]

  ci:
    description: run ci task set
    tasks: ["test", "clippy"]
    parallel: false

  # shell example:
  # clean:
  #   run: "rm -rf ./target"
"#;

#[derive(Debug, Parser)]
#[command(
    name = "otto",
    version = version::VALUE,
    about = "Task runner with run history and notifications",
    styles = clap_styles()
)]
struct Cli {
    #[arg(long = "no-color", global = true)]
    no_color: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init(InitArgs),
    Run(RunArgs),
    History(HistoryArgs),
    Tasks(TasksArgs),
    Validate(ValidateArgs),
    Version,
    Completion(CompletionArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct RunArgs {
    task: Option<String>,
    #[arg(last = true, allow_hyphen_values = true)]
    inline: Vec<String>,

    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(long)]
    name: Option<String>,

    #[arg(long)]
    timeout: Option<String>,

    #[arg(long)]
    retries: Option<i32>,

    #[arg(long = "notify-on")]
    notify_on: Option<String>,

    #[arg(long = "env-file")]
    env_file: Option<PathBuf>,

    #[arg(long = "no-dotenv")]
    no_dotenv: bool,

    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct HistoryArgs {
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    source: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct TasksArgs {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct ValidateArgs {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct CompletionArgs {
    #[arg(value_enum)]
    shell: Shell,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Shell {
    Bash,
    Zsh,
    Fish,
    Powershell,
}

fn clap_styles() -> Styles {
    Styles::plain()
        .header(AnsiColor::White.on_default() | Effects::BOLD)
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .usage(AnsiColor::Cyan.on_default())
        .literal(AnsiColor::Cyan.on_default())
        .placeholder(AnsiColor::Cyan.on_default())
        .valid(AnsiColor::Cyan.on_default())
        .invalid(AnsiColor::Cyan.on_default())
        .context(AnsiColor::White.on_default())
        .context_value(AnsiColor::Cyan.on_default())
}

pub fn run_cli() -> Result<(), AppError> {
    let cli = Cli::parse();
    output::configure(cli.no_color);

    match cli.command {
        Commands::Init(args) => run_init(args),
        Commands::Run(args) => run_run(args),
        Commands::History(args) => run_history(args),
        Commands::Tasks(args) => run_tasks(args),
        Commands::Validate(args) => run_validate(args),
        Commands::Version => {
            println!("{}", version::VALUE);
            Ok(())
        }
        Commands::Completion(args) => run_completion(args),
    }
}

fn run_init(args: InitArgs) -> Result<(), AppError> {
    let config_path = args
        .config
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));

    if config_path.exists() && !args.force {
        return Err(AppError::usage(format!(
            "{} already exists (use --force to overwrite)",
            config_path.display()
        )));
    }

    fs::write(&config_path, DEFAULT_CONFIG_TEMPLATE)
        .map_err(|e| AppError::internal(format!("write {}: {e}", config_path.display())))?;

    println!(
        "created {}",
        output::command(&config_path.display().to_string())
    );
    Ok(())
}

fn run_run(args: RunArgs) -> Result<(), AppError> {
    let config_path = args
        .config
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));

    let dotenv_vars = load_dotenv(
        args.env_file
            .as_deref()
            .unwrap_or_else(|| Path::new(".env")),
        args.no_dotenv,
        args.env_file.is_some(),
    )?;

    if !args.inline.is_empty() {
        if args.task.is_some() {
            return Err(AppError::usage(
                "inline mode requires only command args after --",
            ));
        }

        let (mut resolved, notifications) = resolve_inline_run(
            &args.inline,
            &config_path,
            args.config.is_some(),
            args.name.as_deref(),
            args.timeout.as_deref(),
            args.retries,
            args.notify_on.as_deref(),
        )?;

        apply_runtime_env(&mut resolved, &dotenv_vars);
        return execute_run(resolved, notifications, args.json, true);
    }

    if args.name.is_some()
        || args.timeout.is_some()
        || args.retries.is_some()
        || args.notify_on.is_some()
    {
        return Err(AppError::usage(
            "--name, --timeout, --retries, and --notify-on are inline-only flags; use with 'otto run -- <command>'",
        ));
    }

    let task_name = args
        .task
        .ok_or_else(|| AppError::usage("named task mode requires exactly one task name"))?;

    let cfg = load_config_classified(&config_path)?;
    let notifications = cfg
        .resolve_notification_settings()
        .map_err(AppError::usage)?;

    let mut stack = Vec::new();
    run_named_task(
        &cfg,
        &task_name,
        &notifications,
        args.json,
        &dotenv_vars,
        true,
        &mut stack,
    )
}

fn run_named_task(
    cfg: &Config,
    task_name: &str,
    notifications: &NotificationSettings,
    as_json: bool,
    dotenv_vars: &HashMap<String, String>,
    emit_notifications: bool,
    stack: &mut Vec<String>,
) -> Result<(), AppError> {
    if let Some(index) = stack.iter().position(|name| name == task_name) {
        let mut cycle = stack[index..].to_vec();
        cycle.push(task_name.to_string());
        return Err(AppError::usage(format!(
            "task dependency cycle: {}",
            cycle.join(" -> ")
        )));
    }

    stack.push(task_name.to_string());
    let resolved = cfg.resolve_task(task_name).map_err(AppError::usage)?;
    let result = if resolved.sub_tasks.is_empty() {
        let mut runnable = resolved;
        apply_runtime_env(&mut runnable, dotenv_vars);
        execute_run(runnable, notifications.clone(), as_json, emit_notifications)
    } else {
        execute_task_group(
            cfg,
            resolved,
            notifications,
            as_json,
            dotenv_vars,
            emit_notifications,
            stack,
        )
    };
    stack.pop();
    result
}

fn execute_task_group(
    cfg: &Config,
    resolved: ResolvedTask,
    notifications: &NotificationSettings,
    as_json: bool,
    dotenv_vars: &HashMap<String, String>,
    emit_notifications: bool,
    stack: &mut Vec<String>,
) -> Result<(), AppError> {
    if as_json {
        return Err(AppError::usage(
            "--json is not supported for composed tasks yet",
        ));
    }

    let started_at = OffsetDateTime::now_utc();
    let wall = Instant::now();
    let mut failures: Vec<String> = Vec::new();

    if resolved.parallel {
        let mut handles = Vec::with_capacity(resolved.sub_tasks.len());
        for child in &resolved.sub_tasks {
            let cfg_child = cfg.clone();
            let notifications_child = notifications.clone();
            let dotenv_child = dotenv_vars.clone();
            let mut child_stack = stack.clone();
            let child_name = child.clone();
            handles.push(thread::spawn(move || {
                run_named_task(
                    &cfg_child,
                    &child_name,
                    &notifications_child,
                    false,
                    &dotenv_child,
                    false,
                    &mut child_stack,
                )
                .map_err(|err| format!("{child_name}: {err}"))
            }));
        }

        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(err)) => failures.push(err),
                Err(_) => failures.push("task thread panicked".to_string()),
            }
        }
    } else {
        for child in &resolved.sub_tasks {
            if let Err(err) =
                run_named_task(cfg, child, notifications, false, dotenv_vars, false, stack)
            {
                failures.push(format!("{child}: {err}"));
                break;
            }
        }
    }

    let status = if failures.is_empty() {
        RunStatus::Success
    } else {
        RunStatus::Failed
    };
    let exit_code = if failures.is_empty() { 0 } else { 1 };
    let stderr_tail = if failures.is_empty() {
        None
    } else {
        Some(failures.join("; "))
    };

    let record = RunRecord {
        id: new_record_id(),
        name: resolved.name.clone(),
        source: RunSource::Task,
        command_preview: resolved.command_preview.clone(),
        started_at,
        duration_ms: wall.elapsed().as_millis() as i64,
        exit_code,
        status,
        stderr_tail: stderr_tail.clone(),
    };

    let store = Store::new(DEFAULT_PATH);
    store
        .append(&record)
        .map_err(|err| AppError::internal(err.to_string()))?;

    if emit_notifications && should_notify(&resolved.notify_on, status) {
        let manager = notify::Manager {
            desktop_enabled: notifications.desktop_enabled,
            webhook_url: notifications.webhook_url.clone(),
            webhook_timeout: notifications.webhook_timeout,
        };

        let event = notify::Event {
            name: record.name.clone(),
            source: source_to_str(record.source).to_string(),
            status: status_to_str(record.status).to_string(),
            exit_code: record.exit_code,
            duration: Duration::from_millis(record.duration_ms as u64),
            started_at: record.started_at,
            command_preview: record.command_preview.clone(),
            stderr_tail: record.stderr_tail.clone(),
        };

        if let Err(err) = manager.notify(&event) {
            eprintln!(
                "{} failed to send notification: {err}",
                output::warning("warn")
            );
        }
    }

    if failures.is_empty() {
        let mode = if resolved.parallel {
            "in parallel"
        } else {
            "sequentially"
        };
        println!(
            "{} run \"{}\" finished in {} ({} sub-tasks {})",
            output::success("ok"),
            resolved.name,
            output::number(&output::format_duration_ms(record.duration_ms)),
            resolved.sub_tasks.len(),
            mode
        );
        Ok(())
    } else {
        Err(AppError::runtime(failures.join("; ")))
    }
}

fn resolve_inline_run(
    inline: &[String],
    config_path: &Path,
    explicit_config: bool,
    inline_name: Option<&str>,
    inline_timeout: Option<&str>,
    inline_retries: Option<i32>,
    inline_notify_on: Option<&str>,
) -> Result<(ResolvedTask, NotificationSettings), AppError> {
    let maybe_cfg = maybe_load_config_for_inline(config_path, explicit_config)?;

    let mut defaults = Defaults::default();
    let mut notifications = NotificationSettings {
        desktop_enabled: true,
        webhook_url: String::new(),
        webhook_timeout: Duration::from_secs(5),
    };

    if let Some(cfg) = maybe_cfg {
        defaults = cfg.defaults.clone();
        notifications = cfg
            .resolve_notification_settings()
            .map_err(AppError::usage)?;
    }

    let resolved = config::resolve_inline(
        inline,
        inline_name.unwrap_or_default(),
        inline_timeout.unwrap_or_default(),
        inline_retries,
        inline_notify_on.unwrap_or_default(),
        &defaults,
    )
    .map_err(AppError::usage)?;

    Ok((resolved, notifications))
}

fn maybe_load_config_for_inline(path: &Path, explicit: bool) -> Result<Option<Config>, AppError> {
    if !path.exists() {
        if explicit {
            return Err(AppError::usage(format!(
                "config file {} not found",
                output::command(&path.display().to_string())
            )));
        }
        return Ok(None);
    }

    let cfg = load_config_classified(path)?;
    Ok(Some(cfg))
}

fn execute_run(
    resolved: ResolvedTask,
    notifications: NotificationSettings,
    as_json: bool,
    emit_notifications: bool,
) -> Result<(), AppError> {
    let request = Request {
        name: resolved.name.clone(),
        command_preview: resolved.command_preview.clone(),
        use_shell: resolved.use_shell,
        exec: resolved.exec.clone(),
        shell: resolved.shell.clone(),
        dir: resolved.dir.clone(),
        env: resolved.env.clone(),
        timeout: resolved.timeout,
        retries: resolved.retries,
        retry_backoff: resolved.retry_backoff,
        stream_output: !as_json,
    };

    let execution = runner::execute(&request);
    let (result, run_err) = match execution {
        Ok(ok) => (ok, None),
        Err(err) => (err.result, Some(err.message)),
    };

    let record = RunRecord {
        id: new_record_id(),
        name: resolved.name,
        source: resolved.source,
        command_preview: resolved.command_preview,
        started_at: result.started_at,
        duration_ms: result.duration.as_millis() as i64,
        exit_code: result.exit_code,
        status: result.status,
        stderr_tail: result.stderr_tail,
    };

    let store = Store::new(DEFAULT_PATH);
    store
        .append(&record)
        .map_err(|err| AppError::internal(err.to_string()))?;

    if emit_notifications && should_notify(&resolved.notify_on, record.status) {
        let manager = notify::Manager {
            desktop_enabled: notifications.desktop_enabled,
            webhook_url: notifications.webhook_url,
            webhook_timeout: notifications.webhook_timeout,
        };

        let event = notify::Event {
            name: record.name.clone(),
            source: source_to_str(record.source).to_string(),
            status: status_to_str(record.status).to_string(),
            exit_code: record.exit_code,
            duration: result.duration,
            started_at: record.started_at,
            command_preview: record.command_preview.clone(),
            stderr_tail: record.stderr_tail.clone(),
        };

        if let Err(err) = manager.notify(&event) {
            eprintln!(
                "{} failed to send notification: {err}",
                output::warning("warn")
            );
        }
    }

    if let Some(run_err) = run_err {
        if as_json {
            print_run_json(&record, Some(run_err.clone()))
                .map_err(|e| AppError::internal(format!("encode json: {e}")))?;
        }
        return Err(AppError::runtime(run_err));
    }

    if as_json {
        print_run_json(&record, None)
            .map_err(|e| AppError::internal(format!("encode json: {e}")))?;
        return Ok(());
    }

    println!(
        "{} run \"{}\" finished in {}",
        output::success("ok"),
        record.name,
        output::number(&output::format_duration_ms(record.duration_ms)),
    );

    Ok(())
}

#[derive(Serialize)]
struct RunJsonPayload<'a> {
    id: &'a str,
    name: &'a str,
    source: &'a str,
    command_preview: &'a str,
    #[serde(with = "time::serde::rfc3339")]
    started_at: OffsetDateTime,
    duration_ms: i64,
    exit_code: i32,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr_tail: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

fn print_run_json(record: &RunRecord, error: Option<String>) -> Result<(), io::Error> {
    let payload = RunJsonPayload {
        id: &record.id,
        name: &record.name,
        source: source_to_str(record.source),
        command_preview: &record.command_preview,
        started_at: record.started_at,
        duration_ms: record.duration_ms,
        exit_code: record.exit_code,
        status: status_to_str(record.status),
        stderr_tail: record.stderr_tail.as_deref(),
        error: error.as_deref(),
    };

    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &payload)?;
    writeln!(stdout)
}

fn load_dotenv(
    path: &Path,
    disabled: bool,
    explicit: bool,
) -> Result<HashMap<String, String>, AppError> {
    if disabled {
        return Ok(HashMap::new());
    }

    match crate::envfile::load(path) {
        Ok(vars) => Ok(vars),
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            if explicit {
                Err(AppError::usage(format!(
                    "dotenv file {} not found",
                    output::command(&path.display().to_string())
                )))
            } else {
                Ok(HashMap::new())
            }
        }
        Err(err) => Err(AppError::usage(format!(
            "load dotenv file {}: {}",
            output::command(&path.display().to_string()),
            err
        ))),
    }
}

fn apply_runtime_env(resolved: &mut ResolvedTask, dotenv_vars: &HashMap<String, String>) {
    let mut lookup: HashMap<String, String> = std::env::vars().collect();
    let mut runtime_env: HashMap<String, String> = HashMap::new();

    for (key, value) in dotenv_vars {
        if lookup.contains_key(key) {
            continue;
        }
        runtime_env.insert(key.clone(), value.clone());
        lookup.insert(key.clone(), value.clone());
    }

    if !resolved.env.is_empty() {
        let mut keys: Vec<String> = resolved.env.keys().cloned().collect();
        keys.sort();

        for key in keys {
            if let Some(value) = resolved.env.get(&key) {
                let expanded = expand_variables(value, &lookup);
                runtime_env.insert(key.clone(), expanded.clone());
                lookup.insert(key, expanded);
            }
        }
    }

    if !resolved.dir.is_empty() {
        resolved.dir = expand_variables(&resolved.dir, &lookup);
    }

    if resolved.use_shell {
        resolved.shell = expand_variables(&resolved.shell, &lookup);
        resolved.command_preview = resolved.shell.clone();
    } else if !resolved.exec.is_empty() {
        let expanded: Vec<String> = resolved
            .exec
            .iter()
            .map(|token| expand_variables(token, &lookup))
            .collect();
        resolved.command_preview = expanded.join(" ");
        resolved.exec = expanded;
    }

    resolved.env = runtime_env;
}

fn expand_variables(value: &str, lookup: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'$' {
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }

        if i + 1 >= bytes.len() {
            out.push('$');
            break;
        }

        if bytes[i + 1] == b'{' {
            if let Some(end_rel) = value[i + 2..].find('}') {
                let end = i + 2 + end_rel;
                let key = &value[i + 2..end];
                if let Some(found) = lookup.get(key) {
                    out.push_str(found);
                } else {
                    out.push_str(&format!("${{{key}}}"));
                }
                i = end + 1;
                continue;
            }

            out.push('$');
            i += 1;
            continue;
        }

        let mut j = i + 1;
        while j < bytes.len() {
            let ch = bytes[j] as char;
            if j == i + 1 {
                if !(ch.is_ascii_alphabetic() || ch == '_') {
                    break;
                }
            } else if !(ch.is_ascii_alphanumeric() || ch == '_') {
                break;
            }
            j += 1;
        }

        if j == i + 1 {
            out.push('$');
            i += 1;
            continue;
        }

        let key = &value[i + 1..j];
        if let Some(found) = lookup.get(key) {
            out.push_str(found);
        } else {
            out.push_str(&format!("${{{key}}}"));
        }
        i = j;
    }

    out
}

fn load_config_classified(path: &Path) -> Result<Config, AppError> {
    config::load(path).map_err(|err| {
        if err.starts_with("read config:") && !err.contains("No such file") {
            AppError::internal(err)
        } else {
            AppError::usage(err)
        }
    })
}

fn should_notify(policy: &str, status: RunStatus) -> bool {
    match policy {
        "never" => false,
        "always" => true,
        _ => status == RunStatus::Failed,
    }
}

fn new_record_id() -> String {
    let mut random = [0_u8; 8];
    rand::rng().fill(&mut random);
    let millis = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    format!("{millis}-{}", hex_encode(&random))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn source_to_str(source: RunSource) -> &'static str {
    match source {
        RunSource::Task => "task",
        RunSource::Inline => "inline",
    }
}

fn status_to_str(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Success => "success",
        RunStatus::Failed => "failed",
    }
}

fn run_history(args: HistoryArgs) -> Result<(), AppError> {
    if let Some(status) = &args.status
        && status != "success"
        && status != "failed"
    {
        return Err(AppError::usage("--status must be success or failed"));
    }

    if let Some(source) = &args.source
        && source != "task"
        && source != "inline"
    {
        return Err(AppError::usage("--source must be task or inline"));
    }

    let store = Store::new(DEFAULT_PATH);
    let rows = store.list(&Filter {
        limit: Some(args.limit),
        status: args.status.clone(),
        source: args.source.clone(),
    });

    let rows = rows.map_err(AppError::internal)?;

    if args.json {
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &rows)
            .map_err(|e| AppError::internal(format!("encode history json: {e}")))?;
        writeln!(stdout).map_err(|e| AppError::internal(format!("write output: {e}")))?;
        return Ok(());
    }

    let display_rows: Vec<HistoryRow> = rows
        .into_iter()
        .map(|row| HistoryRow {
            name: row.name,
            source: row.source,
            status: row.status,
            exit_code: row.exit_code,
            started_at: row.started_at,
            duration_ms: row.duration_ms,
        })
        .collect();

    output::print_history(io::stdout().lock(), &display_rows)
        .map_err(|e| AppError::internal(format!("print history: {e}")))
}

fn run_tasks(args: TasksArgs) -> Result<(), AppError> {
    let config_path = args
        .config
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    let cfg = load_config_classified(&config_path)?;
    let tasks = cfg
        .tasks
        .as_ref()
        .ok_or_else(|| AppError::usage("tasks: is required"))?;

    let mut names: Vec<&String> = tasks.keys().collect();
    names.sort();

    #[derive(Serialize)]
    struct TaskJson {
        name: String,
        #[serde(skip_serializing_if = "String::is_empty")]
        description: String,
        command: String,
    }

    let mut items = Vec::with_capacity(names.len());
    for name in names {
        let task = tasks.get(name).expect("task exists");
        let command = if !task.exec.is_empty() {
            task.exec.join(" ")
        } else if !task.tasks.is_empty() {
            let mode = if task.parallel {
                "parallel"
            } else {
                "sequential"
            };
            format!("tasks ({mode}): {}", task.tasks.join(", "))
        } else {
            task.run.clone()
        };

        items.push(TaskJson {
            name: name.clone(),
            description: task.description.clone(),
            command,
        });
    }

    if args.json {
        let mut stdout = io::stdout().lock();
        serde_json::to_writer_pretty(&mut stdout, &items)
            .map_err(|e| AppError::internal(format!("encode tasks json: {e}")))?;
        writeln!(stdout).map_err(|e| AppError::internal(format!("write output: {e}")))?;
        return Ok(());
    }

    let rows: Vec<TaskRow> = items
        .into_iter()
        .map(|item| TaskRow {
            name: item.name,
            description: item.description,
            command: compact_command(&item.command, 100),
        })
        .collect();

    output::print_tasks(io::stdout().lock(), &rows)
        .map_err(|e| AppError::internal(format!("print tasks: {e}")))
}

fn run_validate(args: ValidateArgs) -> Result<(), AppError> {
    #[derive(Serialize)]
    struct Issue<'a> {
        field: &'a str,
        message: &'a str,
    }

    #[derive(Serialize)]
    struct ValidateOutput<'a> {
        valid: bool,
        config: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        issues: Option<Vec<Issue<'a>>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<&'a str>,
    }

    let config_path = args
        .config
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    let config_path_text = config_path.display().to_string();

    let cfg = match config::parse(&config_path) {
        Ok(cfg) => cfg,
        Err(err) => {
            if args.json {
                let output = ValidateOutput {
                    valid: false,
                    config: &config_path_text,
                    issues: None,
                    error: Some(&err),
                };
                let mut stdout = io::stdout().lock();
                serde_json::to_writer_pretty(&mut stdout, &output)
                    .map_err(|e| AppError::internal(format!("encode validate json: {e}")))?;
                writeln!(stdout).map_err(|e| AppError::internal(format!("write output: {e}")))?;
            }
            return Err(AppError::usage(err));
        }
    };

    match config::validate(&cfg) {
        Ok(()) => {
            if args.json {
                let output = ValidateOutput {
                    valid: true,
                    config: &config_path_text,
                    issues: None,
                    error: None,
                };
                let mut stdout = io::stdout().lock();
                serde_json::to_writer_pretty(&mut stdout, &output)
                    .map_err(|e| AppError::internal(format!("encode validate json: {e}")))?;
                writeln!(stdout).map_err(|e| AppError::internal(format!("write output: {e}")))?;
            } else {
                println!("valid {}", output::command(&config_path_text));
            }
            Ok(())
        }
        Err(err) => {
            if args.json {
                let issues: Vec<Issue<'_>> = err
                    .issues
                    .iter()
                    .map(|issue| Issue {
                        field: &issue.field,
                        message: &issue.message,
                    })
                    .collect();
                let output = ValidateOutput {
                    valid: false,
                    config: &config_path_text,
                    issues: Some(issues),
                    error: Some(&err.to_string()),
                };
                let mut stdout = io::stdout().lock();
                serde_json::to_writer_pretty(&mut stdout, &output)
                    .map_err(|e| AppError::internal(format!("encode validate json: {e}")))?;
                writeln!(stdout).map_err(|e| AppError::internal(format!("write output: {e}")))?;
            }
            Err(AppError::usage(err.to_string()))
        }
    }
}

fn compact_command(command: &str, max_chars: usize) -> String {
    let compact = command.split_whitespace().collect::<Vec<_>>().join(" ");

    if max_chars == 0 || compact.chars().count() <= max_chars {
        return compact;
    }

    let limit = max_chars.max(4) - 3;
    format!("{}...", compact.chars().take(limit).collect::<String>())
}

fn run_completion(args: CompletionArgs) -> Result<(), AppError> {
    let mut cmd = Cli::command();
    let mut stdout = io::stdout().lock();

    match args.shell {
        Shell::Bash => generate_completion(clap_complete::shells::Bash, &mut cmd, &mut stdout),
        Shell::Zsh => generate_completion(clap_complete::shells::Zsh, &mut cmd, &mut stdout),
        Shell::Fish => generate_completion(clap_complete::shells::Fish, &mut cmd, &mut stdout),
        Shell::Powershell => {
            generate_completion(clap_complete::shells::PowerShell, &mut cmd, &mut stdout)
        }
    }
    .map_err(|e| AppError::internal(format!("generate completion: {e}")))
}

fn generate_completion<G: Generator>(
    generator: G,
    cmd: &mut clap::Command,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    generate(generator, cmd, "otto", writer);
    writer.flush()
}

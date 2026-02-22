use crate::model::RunStatus;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use wait_timeout::ChildExt;

#[derive(Debug, Clone)]
pub struct Request {
    pub name: String,
    pub command_preview: String,
    pub use_shell: bool,
    pub exec: Vec<String>,
    pub shell: String,
    pub dir: String,
    pub env: HashMap<String, String>,
    pub timeout: Duration,
    pub retries: i32,
    pub retry_backoff: Duration,
    pub stream_output: bool,
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub started_at: OffsetDateTime,
    pub duration: Duration,
    pub exit_code: i32,
    pub status: RunStatus,
    pub stderr_tail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunFailure {
    pub result: RunResult,
    pub message: String,
}

pub fn execute(req: &Request) -> Result<RunResult, RunFailure> {
    if req.retries < 0 {
        return Err(RunFailure {
            result: failed_result(127, Duration::ZERO, None),
            message: "retries must be >= 0".to_string(),
        });
    }

    if req.use_shell && req.shell.trim().is_empty() {
        return Err(RunFailure {
            result: failed_result(127, Duration::ZERO, None),
            message: "shell command is required".to_string(),
        });
    }

    if !req.use_shell && req.exec.is_empty() {
        return Err(RunFailure {
            result: failed_result(127, Duration::ZERO, None),
            message: "exec command is required".to_string(),
        });
    }

    let retry_backoff = if req.retry_backoff.is_zero() {
        Duration::from_secs(1)
    } else {
        req.retry_backoff
    };

    let start = OffsetDateTime::now_utc();
    let wall = Instant::now();
    let attempts = req.retries + 1;

    let mut last_exit = 0;
    let mut last_stderr = None;
    let mut last_error = String::new();

    for attempt in 0..attempts {
        match run_once(req) {
            Ok((code, stderr_tail, None)) => {
                return Ok(RunResult {
                    started_at: start,
                    duration: wall.elapsed(),
                    exit_code: code,
                    status: RunStatus::Success,
                    stderr_tail,
                });
            }
            Ok((code, stderr_tail, Some(err))) => {
                last_exit = code;
                last_stderr = stderr_tail;
                last_error = err;
            }
            Err(err) => {
                last_exit = 127;
                last_stderr = None;
                last_error = err;
            }
        }

        if attempt < attempts - 1 {
            let wait = retry_backoff
                .checked_mul(1_u32 << attempt)
                .unwrap_or(Duration::from_secs(60));
            thread::sleep(wait);
        }
    }

    Err(RunFailure {
        result: RunResult {
            started_at: start,
            duration: wall.elapsed(),
            exit_code: last_exit,
            status: RunStatus::Failed,
            stderr_tail: last_stderr,
        },
        message: last_error,
    })
}

fn run_once(req: &Request) -> Result<(i32, Option<String>, Option<String>), String> {
    let mut command = build_command(req)?;
    if !req.dir.is_empty() {
        command.current_dir(&req.dir);
    }
    if !req.env.is_empty() {
        command.envs(&req.env);
    }

    if req.stream_output {
        command.stdout(Stdio::inherit());
    } else {
        command.stdout(Stdio::null());
    }
    command.stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|e| format!("run command: {e}"))?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "failed to capture stderr".to_string())?;

    let stream_output = req.stream_output;
    let stderr_handle = thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stderr);
        let mut buf = [0_u8; 4096];
        let mut all = Vec::new();
        let mut sink = std::io::stderr().lock();

        loop {
            let read = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };

            let chunk = &buf[..read];
            if stream_output {
                let _ = sink.write_all(chunk);
                let _ = sink.flush();
            }
            all.extend_from_slice(chunk);
        }

        all
    });

    let (status, timeout_hit) = wait_child(&mut child, req.timeout)?;
    let stderr_bytes = stderr_handle
        .join()
        .map_err(|_| "stderr reader thread panicked".to_string())?;

    let stderr_text = String::from_utf8_lossy(&stderr_bytes).to_string();
    let stderr_tail = tail(&stderr_text, 10, 1400);

    if timeout_hit {
        return Ok((
            124,
            stderr_tail,
            Some(format!(
                "command timed out after {}",
                format_duration(req.timeout)
            )),
        ));
    }

    if status.success() {
        return Ok((0, stderr_tail, None));
    }

    let code = status.code().unwrap_or(1);
    Ok((
        code,
        stderr_tail,
        Some(format!("command failed with exit code {code}")),
    ))
}

fn wait_child(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<(ExitStatus, bool), String> {
    if timeout.is_zero() {
        let status = child.wait().map_err(|e| format!("wait command: {e}"))?;
        return Ok((status, false));
    }

    match child
        .wait_timeout(timeout)
        .map_err(|e| format!("wait command: {e}"))?
    {
        Some(status) => Ok((status, false)),
        None => {
            let _ = child.kill();
            let status = child.wait().map_err(|e| format!("wait command: {e}"))?;
            Ok((status, true))
        }
    }
}

fn build_command(req: &Request) -> Result<Command, String> {
    if req.use_shell {
        if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C").arg(&req.shell);
            return Ok(cmd);
        }

        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(&req.shell);
        return Ok(cmd);
    }

    let Some(program) = req.exec.first() else {
        return Err("exec command is required".to_string());
    };

    let mut cmd = Command::new(program);
    if req.exec.len() > 1 {
        cmd.args(&req.exec[1..]);
    }
    Ok(cmd)
}

fn failed_result(exit_code: i32, duration: Duration, stderr_tail: Option<String>) -> RunResult {
    RunResult {
        started_at: OffsetDateTime::now_utc(),
        duration,
        exit_code,
        status: RunStatus::Failed,
        stderr_tail,
    }
}

pub fn tail(input: &str, line_limit: usize, char_limit: usize) -> Option<String> {
    if input.is_empty() {
        return None;
    }

    let trimmed = input.trim_end_matches('\n');
    if trimmed.is_empty() {
        return None;
    }

    let mut lines: Vec<&str> = trimmed.lines().collect();
    if lines.len() > line_limit {
        lines = lines.split_off(lines.len() - line_limit);
    }

    let mut out = lines.join("\n");

    if out.chars().count() > char_limit {
        let start = out.chars().count().saturating_sub(char_limit);
        out = out.chars().skip(start).collect();
    }

    Some(out)
}

fn format_duration(duration: Duration) -> String {
    let ms = duration.as_millis();

    if ms < 1_000 {
        return format!("{ms}ms");
    }

    if ms.is_multiple_of(1_000) {
        return format!("{}s", ms / 1_000);
    }

    format!("{:.3}s", duration.as_secs_f64())
}

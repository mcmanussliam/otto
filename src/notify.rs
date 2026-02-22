use reqwest::blocking::Client;
use serde::Serialize;
use std::process::Command;
use std::time::Duration;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct Event {
    pub name: String,
    pub source: String,
    pub status: String,
    pub exit_code: i32,
    pub duration: Duration,
    pub started_at: OffsetDateTime,
    pub command_preview: String,
    pub stderr_tail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Manager {
    pub desktop_enabled: bool,
    pub webhook_url: String,
    pub webhook_timeout: Duration,
}

impl Manager {
    pub fn notify(&self, event: &Event) -> Result<(), String> {
        let mut errors = Vec::new();

        if self.desktop_enabled
            && let Err(err) = desktop_notify(event)
        {
            errors.push(format!("desktop: {err}"));
        }

        if !self.webhook_url.is_empty()
            && let Err(err) = webhook_notify(&self.webhook_url, self.webhook_timeout, event)
        {
            errors.push(format!("webhook: {err}"));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}

fn desktop_notify(event: &Event) -> Result<(), String> {
    let title = format!("{} {}", event.name, event.status);
    let body = format!(
        "exit {}, duration {}",
        event.exit_code,
        format_duration(event.duration)
    );

    if cfg!(target_os = "macos") {
        let script = format!("display notification {:?} with title {:?}", body, title);

        let status = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .status()
            .map_err(|e| e.to_string())?;

        if status.success() {
            return Ok(());
        }

        return Err(format!("osascript exited with status {status}"));
    }

    if cfg!(target_os = "linux") {
        let status = Command::new("notify-send")
            .arg(&title)
            .arg(&body)
            .status()
            .map_err(|e| e.to_string())?;

        if status.success() {
            return Ok(());
        }

        return Err(format!("notify-send exited with status {status}"));
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct WebhookPayload<'a> {
    name: &'a str,
    source: &'a str,
    status: &'a str,
    exit_code: i32,
    duration_ms: i128,
    started_at: String,
    command_preview: &'a str,
    stderr_tail: &'a str,
}

fn webhook_notify(webhook_url: &str, timeout: Duration, event: &Event) -> Result<(), String> {
    let timeout = if timeout.is_zero() {
        Duration::from_secs(5)
    } else {
        timeout
    };

    let client = Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("build client: {e}"))?;

    let payload = WebhookPayload {
        name: &event.name,
        source: &event.source,
        status: &event.status,
        exit_code: event.exit_code,
        duration_ms: event.duration.as_millis() as i128,
        started_at: event
            .started_at
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|e| format!("format started_at: {e}"))?,
        command_preview: &event.command_preview,
        stderr_tail: event.stderr_tail.as_deref().unwrap_or(""),
    };

    let response = client
        .post(webhook_url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&payload)
        .send()
        .map_err(|e| format!("send request: {e}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("unexpected status {}", response.status().as_u16()))
    }
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

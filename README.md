# Otto

Otto is a simple task runner. Think `make`, but with built-in retries, timeouts, run history, and notifications.

```bash
% otto run -- echo "hello"

ok run "inline" finished in 3ms

% otto history

inline ok success
  source: inline
  exit: 0
  started (UTC): 2026-02-22 18:10:09
  duration: 3ms
```

## Install

Install from crates.io:

```bash
cargo install otto-cli --locked
otto version
```

## How tasks work

Use exactly one command mode per task:

- `exec`: direct argv execution (no shell parsing)
- `run`: shell command (`/bin/sh -c` on macOS/Linux, `cmd /C` on Windows)
- `tasks`: compose other tasks by name

Task example:

```yaml
tasks:
  lint:
    exec: ["cargo", "fmt", "--all", "--check"]

  clippy:
    exec: ["cargo", "clippy", "--all-targets", "--all-features", "--", "-D", "warnings"]

  build:
    exec: ["cargo", "build", "--release"]

  ci:
    tasks: ["lint", "build", "clippy"]
    parallel: false # set true to run child tasks in parallel
```

Shared defaults live in `defaults`, and each task can override:

- `timeout`
- `retries` (0..10)
- `retry_backoff` (uses exponential backoff between attempts)
- `notify_on` (`never`, `failure`, `always`)

## Dotenv and env expansion

`otto run` auto-loads `.env` if present.

- Disable it: `--no-dotenv`
- Use a different file: `--env-file .env.staging`

Variables from process env + dotenv + task `env` are expanded in:

- `run`
- `exec` args
- `dir`
- `env` values

Unknown variables are preserved as `${NAME}`.

## Notifications

Supported channels:

- desktop (`osascript` on macOS, `notify-send` on Linux)
- webhook (`POST` JSON to `notifications.webhook_url`)

`notify_on` controls when notifications fire: `never`, `failure`, `always`.

## History and automation

Every run gets recorded in `.otto/history.jsonl`.

For scripts, use:

- `otto run --json`
- `otto tasks --json`
- `otto history --json`
- `otto validate --json`

In JSON mode, command output is suppressed so stdout is valid JSON only.

## Shell completion

```bash
otto completion bash
otto completion zsh
otto completion fish
otto completion powershell
```

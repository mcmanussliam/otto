# otto

`otto` is a task runner, similar to `make` with retries, timeouts, run history, and
notifications.

```bash
% otto run -- echo "hello"
hello
✔ run "inline" finished in 3ms

% otto history
◉ Recent Runs
NAME    SOURCE  STATUS     EXIT  STARTED (UTC)        DURATION
──────────────────────────────────────────────────────────────
inline  inline  ✔ success     0  2026-02-20 20:01:33       3ms
inline  inline  ✔ success     0  2026-02-20 20:01:05       2ms
```

## Install

### Quick Start (macOS and Linux, recommended)

```bash
brew tap mcmanussliam/otto https://github.com/mcmanussliam/otto
brew install mcmanussliam/otto/otto
otto version
```

### Linux and macOS Installer Script (no Homebrew)

The default script install target is `/usr/local/bin`, which may prompt for `sudo`.

```bash
curl -fsSL https://raw.githubusercontent.com/mcmanussliam/otto/main/scripts/install.sh | sh
otto version
```

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/mcmanussliam/otto/main/scripts/install.ps1 | iex
otto version
```

### Shell Completions

`otto` includes Cobra completions out of the box via:

```bash
otto completion [bash|zsh|fish|powershell]
```

Supported shells:

- zsh
- bash
- fish
- powershell

#### zsh

```bash
mkdir -p ~/.zsh/completions
otto completion zsh > ~/.zsh/completions/_otto
echo 'fpath=(~/.zsh/completions $fpath)' >> ~/.zshrc
echo 'autoload -U compinit && compinit' >> ~/.zshrc
source ~/.zshrc
```

#### bash

```bash
sudo mkdir -p /etc/bash_completion.d
sudo sh -c 'otto completion bash > /etc/bash_completion.d/otto'
source ~/.bashrc
```

User-local alternative (no sudo):

```bash
mkdir -p ~/.local/share/bash-completion/completions
otto completion bash > ~/.local/share/bash-completion/completions/otto
```

#### fish

```bash
mkdir -p ~/.config/fish/completions
otto completion fish > ~/.config/fish/completions/otto.fish
```

#### PowerShell

```powershell
New-Item -ItemType Directory -Force (Split-Path -Parent $PROFILE) | Out-Null
Add-Content -Path $PROFILE -Value 'otto completion powershell | Out-String | Invoke-Expression'
. $PROFILE
```

## Usage

```shell
otto [command]

Commands:
  completion  Generate the autocompletion script for the specified shell
  init      Create a starter otto.yml config
  run       Run a named task or an inline command
  history   Show recent run history
  version   Print otto version

Global flags:
  --no-color  Disable ANSI colors
```

### Initialise Config

```bash
otto init
```

This creates `./otto.yml` with a starter task.

### Run a Task

```bash
otto run test
```

### Run an Inline Command

```bash
otto run -- go test ./...
otto run --name quick --timeout 30s --retries 1 -- go test ./...
```

### Check History

```bash
otto history
otto history --status failed --source inline --limit 10
```

## Config (`otto.yml`)

Starter template:

```yaml
version: 1
defaults:
  timeout: "2m"
  retries: 0
  retry_backoff: "1s"
  notify_on: failure
notifications:
  desktop: true
tasks:
  test:
    description: run unit tests
    exec: ["go", "test", "./..."]
```

Full example:

```yaml
version: 1

defaults:
  timeout: "90s"
  retries: 1
  retry_backoff: "2s"
  notify_on: failure

notifications:
  desktop: true
  webhook_url: "https://example.com/otto-hook"
  webhook_timeout: "5s"

tasks:
  test:
    description: run unit tests
    exec: ["go", "test", "./..."]

  lint:
    description: run linter
    exec: ["golangci-lint", "run"]

  clean-cache:
    description: shell command task
    run: "rm -rf ./.gocache"
```

Validation rules:

- `tasks` is required
- Task names must match `^[a-z0-9][a-z0-9_-]{0,62}$`
- Reserved task names: `init`, `run`, `history`
- Each task must define exactly one of `exec` or `run`
- `retries` must be `0..10`
- Duration fields use Go duration syntax (`500ms`, `30s`, `2m`)
- `notify_on` must be `never`, `failure`, or `always`

## History And Notifications

- Run history is stored in `.otto/history.jsonl`
- Newest entries are shown first in `otto history`
- Notifications can be sent on:
  - desktop (`osascript` on macOS, `notify-send` on Linux)
  - webhook (`POST` JSON to `notifications.webhook_url`)
- Notification policy is controlled by `notify_on`

Webhook payload fields:

```json
{
  "name": "test",
  "source": "task",
  "status": "success",
  "exit_code": 0,
  "duration_ms": 231,
  "started_at": "2026-02-20T19:35:00Z",
  "command_preview": "go test ./...",
  "stderr_tail": ""
}
```

## Development Tasks

This repository now uses `otto.yml` for local development tasks (instead of a
`makefile`).

Run tasks with:

```bash
go run ./cmd/otto run <task>
```

or with a built binary:

```bash
./target/otto run <task>
```

Common development tasks:

- `help`
- `fmt`
- `test`
- `test-race`
- `vet`
- `lint`
- `build`
- `run-cli`
- `coverage`
- `ci`
- `clean`

## Exit Codes

- `0` success
- `1` runtime failure (command failed/timed out)
- `2` usage/config validation error
- `3` internal error

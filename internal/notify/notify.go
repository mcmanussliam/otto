package notify

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os/exec"
	"runtime"
	"strings"
	"time"
)

// Event describes a run notification payload.
type Event struct {
	Name           string
	Source         string
	Status         string
	ExitCode       int
	Duration       time.Duration
	StartedAt      time.Time
	CommandPreview string
	StderrTail     string
}

// Manager dispatches notifications to configured providers.
type Manager struct {
	DesktopEnabled bool
	WebhookURL     string
	WebhookTimeout time.Duration
	HTTPClient     *http.Client
}

// Notify sends an event to all configured providers.
func (m Manager) Notify(ctx context.Context, e Event) error {
	var errs []string

	if m.DesktopEnabled {
		if err := desktopNotify(ctx, e); err != nil {
			errs = append(errs, fmt.Sprintf("desktop: %v", err))
		}
	}

	if m.WebhookURL != "" {
		if err := webhookNotify(ctx, m.WebhookURL, m.WebhookTimeout, m.HTTPClient, e); err != nil {
			errs = append(errs, fmt.Sprintf("webhook: %v", err))
		}
	}

	if len(errs) > 0 {
		return fmt.Errorf("%s", strings.Join(errs, "; "))
	}

	return nil
}

func desktopNotify(ctx context.Context, e Event) error {
	title := fmt.Sprintf("otto: %s %s", e.Name, e.Status)
	body := fmt.Sprintf("exit %d, duration %s", e.ExitCode, e.Duration.Round(time.Millisecond))

	switch runtime.GOOS {
	case "darwin":
		script := fmt.Sprintf(`display notification %q with title %q`, body, title)
		return exec.CommandContext(ctx, "osascript", "-e", script).Run()
	case "linux":
		return exec.CommandContext(ctx, "notify-send", title, body).Run()
	default:
		return nil
	}
}

func webhookNotify(ctx context.Context, webhookURL string, timeout time.Duration, client *http.Client, e Event) error {
	if timeout <= 0 {
		timeout = 5 * time.Second
	}

	if client == nil {
		client = &http.Client{Timeout: timeout}
	}

	payload := map[string]any{
		"name":            e.Name,
		"source":          e.Source,
		"status":          e.Status,
		"exit_code":       e.ExitCode,
		"duration_ms":     e.Duration.Milliseconds(),
		"started_at":      e.StartedAt.UTC().Format(time.RFC3339),
		"command_preview": e.CommandPreview,
		"stderr_tail":     e.StderrTail,
	}

	b, err := json.Marshal(payload)
	if err != nil {
		return fmt.Errorf("marshal payload: %w", err)
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, webhookURL, bytes.NewReader(b))
	if err != nil {
		return fmt.Errorf("build request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")

	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("send request: %w", err)
	}

	defer func() {
		_ = resp.Body.Close()
	}()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return fmt.Errorf("unexpected status %d", resp.StatusCode)
	}

	return nil
}

package config_test

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/mcmanussliam/otto/internal/config"
)

func TestValidateRejectsTaskWithExecAndRun(t *testing.T) {
	cfg := &config.Config{
		Version: 1,
		Tasks: map[string]config.Task{
			"build": {
				Exec: []string{"go", "build", "./..."},
				Run:  "go build ./...",
			},
		},
	}

	err := config.Validate(cfg)
	if err == nil {
		t.Fatal("expected validation error")
	}

	var verr config.ValidationErrors
	if !errors.As(err, &verr) {
		t.Fatalf("expected ValidationErrors, got %T", err)
	}
	if len(verr.Issues) == 0 {
		t.Fatal("expected at least one validation issue")
	}
}

func TestResolveTaskAppliesDefaults(t *testing.T) {
	retries := 2
	cfg := &config.Config{
		Version: 1,
		Defaults: config.Defaults{
			Timeout:      "3s",
			Retries:      &retries,
			RetryBackoff: "2s",
			NotifyOn:     "always",
		},
		Tasks: map[string]config.Task{
			"test": {
				Exec: []string{"go", "test", "./..."},
			},
		},
	}

	got, err := cfg.ResolveTask("test")
	if err != nil {
		t.Fatalf("ResolveTask failed: %v", err)
	}

	if got.Timeout != 3*time.Second {
		t.Fatalf("timeout mismatch: got %s", got.Timeout)
	}
	if got.Retries != 2 {
		t.Fatalf("retries mismatch: got %d", got.Retries)
	}
	if got.RetryBackoff != 2*time.Second {
		t.Fatalf("retry backoff mismatch: got %s", got.RetryBackoff)
	}
	if got.NotifyOn != "always" {
		t.Fatalf("notify_on mismatch: got %q", got.NotifyOn)
	}
}

func TestResolveInlineUsesDefaultsAndOverrides(t *testing.T) {
	retries := 3
	defaults := config.Defaults{
		Timeout:      "4s",
		Retries:      &retries,
		RetryBackoff: "2s",
		NotifyOn:     "always",
	}

	got, err := config.ResolveInline([]string{"go", "test", "./..."}, "", "", -1, "", defaults)
	if err != nil {
		t.Fatalf("ResolveInline failed: %v", err)
	}

	if got.Name != "inline" {
		t.Fatalf("expected default name inline, got %q", got.Name)
	}
	if got.Timeout != 4*time.Second {
		t.Fatalf("timeout mismatch: got %s", got.Timeout)
	}
	if got.Retries != 3 {
		t.Fatalf("retries mismatch: got %d", got.Retries)
	}
	if got.NotifyOn != "always" {
		t.Fatalf("notify_on mismatch: got %q", got.NotifyOn)
	}

	override, err := config.ResolveInline([]string{"echo", "ok"}, "quick", "1s", 1, "failure", defaults)
	if err != nil {
		t.Fatalf("ResolveInline override failed: %v", err)
	}
	if override.Name != "quick" {
		t.Fatalf("name mismatch: got %q", override.Name)
	}
	if override.Timeout != time.Second {
		t.Fatalf("timeout override mismatch: got %s", override.Timeout)
	}
	if override.Retries != 1 {
		t.Fatalf("retry override mismatch: got %d", override.Retries)
	}
	if override.NotifyOn != "failure" {
		t.Fatalf("notify_on override mismatch: got %q", override.NotifyOn)
	}
}

func TestValidateRejectsInvalidTaskNameAndReservedName(t *testing.T) {
	cfg := &config.Config{
		Version: 1,
		Tasks: map[string]config.Task{
			"BadName": {Exec: []string{"echo", "x"}},
			"run":     {Exec: []string{"echo", "y"}},
		},
	}

	err := config.Validate(cfg)
	if err == nil {
		t.Fatal("expected validation error")
	}
}

func TestLoadRejectsUnknownField(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "otto.yml")

	content := `version: 1
tasks:
  test:
    exec: ["echo", "ok"]
    unexpected: true
`

	if err := os.WriteFile(path, []byte(strings.TrimSpace(content)+"\n"), 0o644); err != nil {
		t.Fatalf("write config: %v", err)
	}

	_, err := config.Load(path)
	if err == nil {
		t.Fatal("expected load error for unknown field")
	}
}

func TestResolveNotificationSettingsDefaultsAndOverride(t *testing.T) {
	cfg := &config.Config{}

	settings, err := cfg.ResolveNotificationSettings()
	if err != nil {
		t.Fatalf("resolve default notifications: %v", err)
	}

	if !settings.DesktopEnabled {
		t.Fatal("expected desktop notifications enabled by default")
	}

	if settings.WebhookTimeout != 5*time.Second {
		t.Fatalf("default webhook timeout mismatch: %s", settings.WebhookTimeout)
	}

	desktop := false
	cfg.Notifications = config.Notifications{
		Desktop:        &desktop,
		WebhookURL:     "https://example.com/hook",
		WebhookTimeout: "2s",
	}

	override, err := cfg.ResolveNotificationSettings()
	if err != nil {
		t.Fatalf("resolve overridden notifications: %v", err)
	}

	if override.DesktopEnabled {
		t.Fatal("expected desktop notifications disabled")
	}

	if override.WebhookTimeout != 2*time.Second {
		t.Fatalf("override webhook timeout mismatch: %s", override.WebhookTimeout)
	}
}

func TestResolveInlineRejectsInvalidRetries(t *testing.T) {
	_, err := config.ResolveInline([]string{"echo", "ok"}, "", "", 11, "", config.Defaults{})
	if err == nil {
		t.Fatal("expected invalid retries error")
	}
}

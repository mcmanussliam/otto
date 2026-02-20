package runner_test

import (
	"context"
	"path/filepath"
	"testing"
	"time"

	"github.com/mcmanussliam/otto/internal/model"
	"github.com/mcmanussliam/otto/internal/runner"
)

func TestRunnerExecuteSuccess(t *testing.T) {
	result, err := runner.Execute(context.Background(), runner.Request{
		Name:         "success",
		Source:       model.SourceInline,
		UseShell:     false,
		Exec:         []string{"/bin/sh", "-c", "echo ok"},
		RetryBackoff: 10 * time.Millisecond,
	})
	if err != nil {
		t.Fatalf("expected success, got error: %v", err)
	}

	if result.ExitCode != 0 {
		t.Fatalf("expected exit code 0, got %d", result.ExitCode)
	}

	if result.Status != model.StatusSuccess {
		t.Fatalf("expected success status, got %s", result.Status)
	}
}

func TestRunnerExecuteFailure(t *testing.T) {
	result, err := runner.Execute(context.Background(), runner.Request{
		Name:         "fail",
		Source:       model.SourceInline,
		UseShell:     false,
		Exec:         []string{"/bin/sh", "-c", "echo err >&2; exit 7"},
		RetryBackoff: 10 * time.Millisecond,
	})
	if err == nil {
		t.Fatal("expected failure error")
	}

	if result.ExitCode != 7 {
		t.Fatalf("expected exit code 7, got %d", result.ExitCode)
	}

	if result.Status != model.StatusFailed {
		t.Fatalf("expected failed status, got %s", result.Status)
	}
}

func TestRunnerExecuteTimeout(t *testing.T) {
	result, err := runner.Execute(context.Background(), runner.Request{
		Name:         "timeout",
		Source:       model.SourceInline,
		UseShell:     false,
		Exec:         []string{"/bin/sh", "-c", "sleep 1"},
		Timeout:      50 * time.Millisecond,
		RetryBackoff: 10 * time.Millisecond,
	})
	if err == nil {
		t.Fatal("expected timeout error")
	}

	if result.ExitCode != 124 {
		t.Fatalf("expected timeout exit code 124, got %d", result.ExitCode)
	}
}

func TestRunnerExecuteRetryThenSuccess(t *testing.T) {
	dir := t.TempDir()
	flag := filepath.Join(dir, "flag")
	script := "[ -f " + flag + " ] || { touch " + flag + "; exit 9; }; exit 0"

	result, err := runner.Execute(context.Background(), runner.Request{
		Name:         "retry",
		Source:       model.SourceInline,
		UseShell:     true,
		Shell:        script,
		Retries:      1,
		RetryBackoff: 10 * time.Millisecond,
	})
	if err != nil {
		t.Fatalf("expected retry success, got error: %v", err)
	}

	if result.ExitCode != 0 {
		t.Fatalf("expected exit code 0, got %d", result.ExitCode)
	}
}

func TestRunnerValidateRequest(t *testing.T) {
	_, err := runner.Execute(context.Background(), runner.Request{
		Name:    "invalid",
		Source:  model.SourceInline,
		Retries: -1,
	})
	if err == nil {
		t.Fatal("expected retries validation error")
	}
}

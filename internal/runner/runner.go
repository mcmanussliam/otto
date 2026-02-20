package runner

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"runtime"
	"strings"
	"time"

	"github.com/mcmanussliam/otto/internal/model"
)

// Request defines one run execution request.
type Request struct {
	Name           string
	Source         model.RunSource
	CommandPreview string
	UseShell       bool
	Exec           []string
	Shell          string
	Dir            string
	Env            map[string]string
	Timeout        time.Duration
	Retries        int
	RetryBackoff   time.Duration
}

// Result captures the final execution outcome.
type Result struct {
	StartedAt  time.Time
	Duration   time.Duration
	ExitCode   int
	Status     model.RunStatus
	StderrTail string
}

// Execute runs a command with timeout and retry handling.
func Execute(ctx context.Context, req Request) (Result, error) {
	if req.Retries < 0 {
		return Result{}, errors.New("retries must be >= 0")
	}

	if req.RetryBackoff <= 0 {
		req.RetryBackoff = time.Second
	}

	if req.UseShell && req.Shell == "" {
		return Result{}, errors.New("shell command is required")
	}

	if !req.UseShell && len(req.Exec) == 0 {
		return Result{}, errors.New("exec command is required")
	}

	start := time.Now().UTC()
	attempts := req.Retries + 1

	var finalErr error
	var finalCode int
	var stderrTail string

	for attempt := 0; attempt < attempts; attempt++ {
		code, stderr, err := runOnce(ctx, req)
		stderrTail = stderr
		finalCode = code
		finalErr = err

		if err == nil {
			return Result{
				StartedAt:  start,
				Duration:   time.Since(start),
				ExitCode:   0,
				Status:     model.StatusSuccess,
				StderrTail: stderrTail,
			}, nil
		}

		if attempt == attempts-1 {
			break
		}

		wait := req.RetryBackoff << attempt
		canceled := false
		select {
		case <-ctx.Done():
			canceled = true
		case <-time.After(wait):
		}
		if canceled {
			break
		}
	}

	return Result{
		StartedAt:  start,
		Duration:   time.Since(start),
		ExitCode:   finalCode,
		Status:     model.StatusFailed,
		StderrTail: stderrTail,
	}, finalErr
}

func runOnce(ctx context.Context, req Request) (int, string, error) {
	execCtx := ctx
	var cancel context.CancelFunc
	if req.Timeout > 0 {
		execCtx, cancel = context.WithTimeout(ctx, req.Timeout)
		defer cancel()
	}

	cmd, err := buildCommand(execCtx, req)
	if err != nil {
		return 127, "", err
	}

	cmd.Dir = req.Dir
	if len(req.Env) > 0 {
		cmd.Env = append(os.Environ(), flattenEnv(req.Env)...)
	}

	var stderrBuf bytes.Buffer
	cmd.Stdout = os.Stdout
	cmd.Stderr = io.MultiWriter(os.Stderr, &stderrBuf)

	runErr := cmd.Run()
	stderrTail := tail(stderrBuf.String(), 10, 1400)

	if runErr == nil {
		return 0, stderrTail, nil
	}

	if errors.Is(execCtx.Err(), context.DeadlineExceeded) {
		return 124, stderrTail, fmt.Errorf("command timed out after %s", req.Timeout)
	}

	var exitErr *exec.ExitError
	if errors.As(runErr, &exitErr) {
		return exitErr.ExitCode(), stderrTail, fmt.Errorf("command failed with exit code %d", exitErr.ExitCode())
	}

	return 127, stderrTail, fmt.Errorf("run command: %w", runErr)
}

func buildCommand(ctx context.Context, req Request) (*exec.Cmd, error) {
	if req.UseShell {
		if runtime.GOOS == "windows" {
			return exec.CommandContext(ctx, "cmd", "/C", req.Shell), nil
		}

		return exec.CommandContext(ctx, "/bin/sh", "-c", req.Shell), nil
	}

	return exec.CommandContext(ctx, req.Exec[0], req.Exec[1:]...), nil
}

func flattenEnv(env map[string]string) []string {
	out := make([]string, 0, len(env))
	for k, v := range env {
		out = append(out, fmt.Sprintf("%s=%s", k, v))
	}

	return out
}

func tail(s string, lineLimit, charLimit int) string {
	if s == "" {
		return ""
	}

	lines := strings.Split(strings.TrimRight(s, "\n"), "\n")
	if len(lines) > lineLimit {
		lines = lines[len(lines)-lineLimit:]
	}

	out := strings.Join(lines, "\n")
	if len(out) > charLimit {
		out = out[len(out)-charLimit:]
	}

	return out
}

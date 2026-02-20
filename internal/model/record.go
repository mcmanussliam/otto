package model

import "time"

// RunSource describes where a run request originated.
type RunSource string

const (
	// SourceTask indicates a run triggered from a named config task.
	SourceTask RunSource = "task"
	// SourceInline indicates a run triggered from inline command arguments.
	SourceInline RunSource = "inline"
)

// RunStatus describes the final state of a run.
type RunStatus string

const (
	// StatusSuccess indicates command execution completed successfully.
	StatusSuccess RunStatus = "success"
	// StatusFailed indicates command execution failed or timed out.
	StatusFailed RunStatus = "failed"
)

// RunRecord is a persisted execution history entry.
type RunRecord struct {
	ID             string    `json:"id"`
	Name           string    `json:"name"`
	Source         RunSource `json:"source"`
	CommandPreview string    `json:"command_preview"`
	StartedAt      time.Time `json:"started_at"`
	DurationMs     int64     `json:"duration_ms"`
	ExitCode       int       `json:"exit_code"`
	Status         RunStatus `json:"status"`
	StderrTail     string    `json:"stderr_tail,omitempty"`
}

package output_test

import (
	"bytes"
	"strings"
	"testing"
	"time"

	"github.com/mcmanussliam/otto/internal/model"
	"github.com/mcmanussliam/otto/internal/output"
)

func TestPrintHistoryEmpty(t *testing.T) {
	var buf bytes.Buffer
	output.SetColorEnabled(false)
	t.Cleanup(func() { output.SetColorEnabled(true) })

	output.PrintHistory(&buf, nil)

	text := buf.String()
	if !strings.Contains(text, "No run history yet") {
		t.Fatalf("unexpected empty output: %q", text)
	}
}

func TestPrintHistoryRows(t *testing.T) {
	var buf bytes.Buffer
	output.SetColorEnabled(false)
	t.Cleanup(func() { output.SetColorEnabled(true) })

	rows := []model.RunRecord{
		{
			Name:       "inline",
			Source:     model.SourceInline,
			Status:     model.StatusSuccess,
			ExitCode:   0,
			StartedAt:  time.Date(2026, 2, 20, 0, 0, 0, 0, time.UTC),
			DurationMs: 25,
		},
	}

	output.PrintHistory(&buf, rows)

	text := buf.String()
	if !strings.Contains(text, "Recent Runs") {
		t.Fatalf("expected header in output: %q", text)
	}

	if !strings.Contains(text, "inline") {
		t.Fatalf("expected row in output: %q", text)
	}

	if !strings.Contains(text, "✔ success") {
		t.Fatalf("expected styled status label in output: %q", text)
	}
}

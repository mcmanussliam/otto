package output

import (
	"fmt"
	"io"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/mcmanussliam/otto/internal/model"
)

// PrintHistory renders history rows in a readable table.
func PrintHistory(w io.Writer, rows []model.RunRecord) {
	if len(rows) == 0 {
		_, _ = fmt.Fprintf(w, "%s %s\n", Info("ℹ"), Muted("No run history yet."))
		return
	}

	type row struct {
		name     string
		source   string
		status   string
		exit     string
		started  string
		duration string
	}

	rendered := make([]row, 0, len(rows))
	nameW := utf8.RuneCountInString("NAME")
	sourceW := utf8.RuneCountInString("SOURCE")
	statusW := utf8.RuneCountInString("STATUS")
	exitW := utf8.RuneCountInString("EXIT")
	startedW := utf8.RuneCountInString("STARTED (UTC)")
	durationW := utf8.RuneCountInString("DURATION")

	for _, rec := range rows {
		r := row{
			name:     rec.Name,
			source:   string(rec.Source),
			status:   statusLabel(rec.Status),
			exit:     strconv.Itoa(rec.ExitCode),
			started:  rec.StartedAt.UTC().Format("2006-01-02 15:04:05"),
			duration: (time.Duration(rec.DurationMs) * time.Millisecond).String(),
		}

		rendered = append(rendered, r)
		nameW = max(nameW, utf8.RuneCountInString(r.name))
		sourceW = max(sourceW, utf8.RuneCountInString(r.source))
		statusW = max(statusW, utf8.RuneCountInString(r.status))
		exitW = max(exitW, utf8.RuneCountInString(r.exit))
		startedW = max(startedW, utf8.RuneCountInString(r.started))
		durationW = max(durationW, utf8.RuneCountInString(r.duration))
	}

	_, _ = fmt.Fprintf(w, "%s %s\n", Accent("◉"), Bold("Recent Runs"))

	header := fmt.Sprintf(
		"%s  %s  %s  %s  %s  %s",
		padRight("NAME", nameW),
		padRight("SOURCE", sourceW),
		padRight("STATUS", statusW),
		padLeft("EXIT", exitW),
		padRight("STARTED (UTC)", startedW),
		padLeft("DURATION", durationW),
	)
	_, _ = fmt.Fprintln(w, Bold(header))
	_, _ = fmt.Fprintln(w, Muted(strings.Repeat("─", utf8.RuneCountInString(header))))

	for _, rec := range rendered {
		_, _ = fmt.Fprintf(
			w,
			"%s  %s  %s  %s  %s  %s\n",
			padRight(rec.name, nameW),
			styleSource(padRight(rec.source, sourceW), rec.source),
			styleStatus(padRight(rec.status, statusW), rec.status),
			styleExit(padLeft(rec.exit, exitW), rec.exit),
			Muted(padRight(rec.started, startedW)),
			Accent(padLeft(rec.duration, durationW)),
		)
	}
}

func statusLabel(status model.RunStatus) string {
	switch status {
	case model.StatusSuccess:
		return "✔ success"
	case model.StatusFailed:
		return "✖ failed"
	default:
		return string(status)
	}
}

func styleSource(text, source string) string {
	switch source {
	case string(model.SourceTask):
		return Info(text)
	case string(model.SourceInline):
		return Accent(text)
	default:
		return text
	}
}

func styleStatus(text, status string) string {
	if strings.Contains(status, "success") {
		return Success(text)
	}
	if strings.Contains(status, "failed") {
		return Failure(text)
	}
	return text
}

func styleExit(text, exit string) string {
	if exit == "0" {
		return Success(text)
	}
	return Failure(text)
}

func padRight(value string, width int) string {
	diff := width - utf8.RuneCountInString(value)
	if diff <= 0 {
		return value
	}
	return value + strings.Repeat(" ", diff)
}

func padLeft(value string, width int) string {
	diff := width - utf8.RuneCountInString(value)
	if diff <= 0 {
		return value
	}
	return strings.Repeat(" ", diff) + value
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}

package history_test

import (
	"context"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/mcmanussliam/otto/internal/history"
	"github.com/mcmanussliam/otto/internal/model"
)

func TestStoreAppendAndList(t *testing.T) {
	path := filepath.Join(t.TempDir(), "history.jsonl")
	store := history.NewStore(path)

	first := model.RunRecord{
		ID:         "1",
		Name:       "test",
		Source:     model.SourceTask,
		StartedAt:  time.Now().Add(-time.Minute),
		DurationMs: 1000,
		ExitCode:   0,
		Status:     model.StatusSuccess,
	}
	second := model.RunRecord{
		ID:         "2",
		Name:       "inline",
		Source:     model.SourceInline,
		StartedAt:  time.Now(),
		DurationMs: 500,
		ExitCode:   1,
		Status:     model.StatusFailed,
	}

	if err := store.Append(context.Background(), first); err != nil {
		t.Fatalf("append first: %v", err)
	}
	if err := store.Append(context.Background(), second); err != nil {
		t.Fatalf("append second: %v", err)
	}

	rows, err := store.List(context.Background(), history.Filter{Limit: 10})
	if err != nil {
		t.Fatalf("list history: %v", err)
	}
	if len(rows) != 2 {
		t.Fatalf("expected 2 rows, got %d", len(rows))
	}
	if rows[0].ID != "2" {
		t.Fatalf("expected newest row first, got %s", rows[0].ID)
	}

	filtered, err := store.List(context.Background(), history.Filter{Status: "failed"})
	if err != nil {
		t.Fatalf("list filtered history: %v", err)
	}
	if len(filtered) != 1 {
		t.Fatalf("expected 1 filtered row, got %d", len(filtered))
	}
	if filtered[0].ID != "2" {
		t.Fatalf("unexpected filtered row id %s", filtered[0].ID)
	}
}

func TestStoreListIgnoresMalformedLines(t *testing.T) {
	path := filepath.Join(t.TempDir(), "history.jsonl")
	store := history.NewStore(path)

	valid := model.RunRecord{
		ID:         "good",
		Name:       "task",
		Source:     model.SourceTask,
		StartedAt:  time.Now(),
		DurationMs: 10,
		ExitCode:   0,
		Status:     model.StatusSuccess,
	}

	if err := store.Append(context.Background(), valid); err != nil {
		t.Fatalf("append valid line: %v", err)
	}

	file, err := os.OpenFile(path, os.O_APPEND|os.O_WRONLY, 0o600)
	if err != nil {
		t.Fatalf("open history file: %v", err)
	}

	_, _ = file.WriteString("{invalid-json}\n")
	_ = file.Close()

	rows, err := store.List(context.Background(), history.Filter{})
	if err != nil {
		t.Fatalf("list history: %v", err)
	}

	if len(rows) != 1 {
		t.Fatalf("expected 1 valid row, got %d", len(rows))
	}
}

func TestStoreListFiltersSourceAndLimit(t *testing.T) {
	path := filepath.Join(t.TempDir(), "history.jsonl")
	store := history.NewStore(path)

	records := []model.RunRecord{
		{
			ID:         "a",
			Name:       "task-a",
			Source:     model.SourceTask,
			StartedAt:  time.Now().Add(-3 * time.Minute),
			DurationMs: 50,
			ExitCode:   0,
			Status:     model.StatusSuccess,
		},
		{
			ID:         "b",
			Name:       "inline-b",
			Source:     model.SourceInline,
			StartedAt:  time.Now().Add(-2 * time.Minute),
			DurationMs: 50,
			ExitCode:   0,
			Status:     model.StatusSuccess,
		},
		{
			ID:         "c",
			Name:       "inline-c",
			Source:     model.SourceInline,
			StartedAt:  time.Now().Add(-1 * time.Minute),
			DurationMs: 50,
			ExitCode:   0,
			Status:     model.StatusSuccess,
		},
	}

	for _, rec := range records {
		if err := store.Append(context.Background(), rec); err != nil {
			t.Fatalf("append record %s: %v", rec.ID, err)
		}
	}

	rows, err := store.List(context.Background(), history.Filter{
		Source: "inline",
		Limit:  1,
	})
	if err != nil {
		t.Fatalf("list filtered history: %v", err)
	}

	if len(rows) != 1 {
		t.Fatalf("expected 1 row with limit, got %d", len(rows))
	}

	if rows[0].Source != model.SourceInline {
		t.Fatalf("expected inline source, got %s", rows[0].Source)
	}

	if rows[0].ID != "c" {
		t.Fatalf("expected newest inline row c, got %s", rows[0].ID)
	}
}

func TestStoreListMissingFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "missing-history.jsonl")
	store := history.NewStore(path)

	rows, err := store.List(context.Background(), history.Filter{})
	if err != nil {
		t.Fatalf("list on missing file should not fail: %v", err)
	}

	if len(rows) != 0 {
		t.Fatalf("expected no rows on missing file, got %d", len(rows))
	}
}

package history

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"slices"
	"strings"

	"github.com/mcmanussliam/otto/internal/model"
)

// DefaultPath is the default JSONL history location.
const DefaultPath = ".otto/history.jsonl"

// Filter defines history query filters.
type Filter struct {
	Limit  int
	Status string
	Source string
}

// Store reads and writes run history records.
type Store struct {
	path string
}

// NewStore creates a history store at path.
func NewStore(path string) *Store {
	return &Store{path: path}
}

// Append writes one history record to storage.
func (s *Store) Append(_ context.Context, record model.RunRecord) error {
	if err := os.MkdirAll(filepath.Dir(s.path), 0o700); err != nil {
		return fmt.Errorf("create history directory: %w", err)
	}

	f, err := os.OpenFile(s.path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o600)
	if err != nil {
		return fmt.Errorf("open history file: %w", err)
	}

	defer func() {
		_ = f.Close()
	}()

	b, err := json.Marshal(record)
	if err != nil {
		return fmt.Errorf("serialize history record: %w", err)
	}

	if _, err := f.Write(append(b, '\n')); err != nil {
		return fmt.Errorf("write history record: %w", err)
	}

	return nil
}

// List returns history records with filters applied.
func (s *Store) List(_ context.Context, filter Filter) ([]model.RunRecord, error) {
	f, err := os.Open(s.path)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, nil
		}

		return nil, fmt.Errorf("open history file: %w", err)
	}

	defer func() {
		_ = f.Close()
	}()

	scanner := bufio.NewScanner(f)
	records := make([]model.RunRecord, 0, 64)

	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}

		var rec model.RunRecord
		if err := json.Unmarshal([]byte(line), &rec); err != nil {
			// Ignore malformed line to keep history resilient.
			continue
		}

		if !matchesFilter(rec, filter) {
			continue
		}

		records = append(records, rec)
	}

	if err := scanner.Err(); err != nil && !errors.Is(err, io.EOF) {
		return nil, fmt.Errorf("scan history file: %w", err)
	}

	slices.Reverse(records)
	if filter.Limit > 0 && len(records) > filter.Limit {
		records = records[:filter.Limit]
	}

	return records, nil
}

func matchesFilter(rec model.RunRecord, filter Filter) bool {
	if filter.Status != "" && string(rec.Status) != filter.Status {
		return false
	}
	if filter.Source != "" && string(rec.Source) != filter.Source {
		return false
	}
	return true
}

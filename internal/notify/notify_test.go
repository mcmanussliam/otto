package notify_test

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"strings"
	"testing"
	"time"

	"github.com/mcmanussliam/otto/internal/notify"
)

func TestNotifyWebhookSuccess(t *testing.T) {
	var received map[string]any

	client := &http.Client{
		Transport: roundTripperFunc(func(r *http.Request) (*http.Response, error) {
			defer func() {
				_ = r.Body.Close()
			}()

			if r.Method != http.MethodPost {
				t.Fatalf("expected POST, got %s", r.Method)
			}

			if r.Header.Get("Content-Type") != "application/json" {
				t.Fatalf("unexpected content type %s", r.Header.Get("Content-Type"))
			}

			if err := json.NewDecoder(r.Body).Decode(&received); err != nil {
				t.Fatalf("decode webhook payload: %v", err)
			}

			return &http.Response{
				StatusCode: http.StatusOK,
				Body:       io.NopCloser(strings.NewReader("ok")),
				Header:     make(http.Header),
			}, nil
		}),
	}

	manager := notify.Manager{
		DesktopEnabled: false,
		WebhookURL:     "http://example.test/webhook",
		WebhookTimeout: time.Second,
		HTTPClient:     client,
	}

	err := manager.Notify(context.Background(), notify.Event{
		Name:           "inline",
		Source:         "inline",
		Status:         "success",
		ExitCode:       0,
		Duration:       500 * time.Millisecond,
		StartedAt:      time.Now(),
		CommandPreview: "echo ok",
		StderrTail:     "",
	})
	if err != nil {
		t.Fatalf("expected notify success, got %v", err)
	}

	if received["name"] != "inline" {
		t.Fatalf("unexpected payload name %v", received["name"])
	}
}

func TestNotifyWebhookFailureStatus(t *testing.T) {
	client := &http.Client{
		Transport: roundTripperFunc(func(r *http.Request) (*http.Response, error) {
			return &http.Response{
				StatusCode: http.StatusBadRequest,
				Body:       io.NopCloser(strings.NewReader("bad request")),
				Header:     make(http.Header),
			}, nil
		}),
	}

	manager := notify.Manager{
		DesktopEnabled: false,
		WebhookURL:     "http://example.test/webhook",
		WebhookTimeout: time.Second,
		HTTPClient:     client,
	}

	err := manager.Notify(context.Background(), notify.Event{
		Name:      "inline",
		Source:    "inline",
		Status:    "failed",
		ExitCode:  1,
		Duration:  time.Second,
		StartedAt: time.Now(),
	})
	if err == nil {
		t.Fatal("expected notify failure on non-2xx status")
	}
}

func TestNotifyNoProviders(t *testing.T) {
	manager := notify.Manager{
		DesktopEnabled: false,
	}

	err := manager.Notify(context.Background(), notify.Event{
		Name:      "test",
		Source:    "task",
		Status:    "success",
		ExitCode:  0,
		Duration:  time.Second,
		StartedAt: time.Now(),
	})
	if err != nil {
		t.Fatalf("expected no-provider notify to succeed, got %v", err)
	}
}

type roundTripperFunc func(*http.Request) (*http.Response, error)

func (fn roundTripperFunc) RoundTrip(r *http.Request) (*http.Response, error) {
	return fn(r)
}

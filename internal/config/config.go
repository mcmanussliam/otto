package config

import (
	"bytes"
	"errors"
	"fmt"
	"net/url"
	"os"
	"regexp"
	"slices"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

const (
	// CurrentVersion is the only supported config version in MVP.
	CurrentVersion = 1
)

var (
	taskNamePattern = regexp.MustCompile(`^[a-z0-9][a-z0-9_-]{0,62}$`)
	reservedNames   = []string{"init", "run", "history"}
	validNotifyOn   = []string{"never", "failure", "always"}
)

// Config is the root otto.yml schema.
type Config struct {
	Version       int             `yaml:"version"`
	Defaults      Defaults        `yaml:"defaults"`
	Notifications Notifications   `yaml:"notifications"`
	Tasks         map[string]Task `yaml:"tasks"`
}

// Defaults defines root-level default execution settings.
type Defaults struct {
	Timeout      string `yaml:"timeout"`
	Retries      *int   `yaml:"retries"`
	RetryBackoff string `yaml:"retry_backoff"`
	NotifyOn     string `yaml:"notify_on"`
}

// Notifications defines root-level notifier settings.
type Notifications struct {
	Desktop        *bool  `yaml:"desktop"`
	WebhookURL     string `yaml:"webhook_url"`
	WebhookTimeout string `yaml:"webhook_timeout"`
}

// Task defines a named task configuration.
type Task struct {
	Description  string            `yaml:"description"`
	Exec         []string          `yaml:"exec"`
	Run          string            `yaml:"run"`
	Dir          string            `yaml:"dir"`
	Env          map[string]string `yaml:"env"`
	Timeout      string            `yaml:"timeout"`
	Retries      *int              `yaml:"retries"`
	RetryBackoff string            `yaml:"retry_backoff"`
	NotifyOn     string            `yaml:"notify_on"`
}

// ResolvedTask is a fully resolved, validated execution definition.
type ResolvedTask struct {
	Name           string
	Source         string
	CommandPreview string
	UseShell       bool
	Exec           []string
	Shell          string
	Dir            string
	Env            map[string]string
	Timeout        time.Duration
	Retries        int
	RetryBackoff   time.Duration
	NotifyOn       string
}

// NotificationSettings is the resolved notification provider configuration.
type NotificationSettings struct {
	DesktopEnabled bool
	WebhookURL     string
	WebhookTimeout time.Duration
}

// ValidationError captures one schema validation failure.
type ValidationError struct {
	Field   string
	Message string
}

// Error implements error.
func (e ValidationError) Error() string {
	return fmt.Sprintf("%s: %s", e.Field, e.Message)
}

// ValidationErrors groups multiple validation failures.
type ValidationErrors struct {
	Issues []ValidationError
}

// Error implements error.
func (e ValidationErrors) Error() string {
	if len(e.Issues) == 0 {
		return "configuration validation failed"
	}
	return fmt.Sprintf("configuration validation failed: %s", e.Issues[0].Error())
}

// HasIssues reports whether validation captured at least one issue.
func (e ValidationErrors) HasIssues() bool {
	return len(e.Issues) > 0
}

// Load parses and validates a config file from path.
func Load(path string) (*Config, error) {
	b, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read config: %w", err)
	}

	dec := yaml.NewDecoder(bytes.NewReader(b))
	dec.KnownFields(true)

	var cfg Config
	if err := dec.Decode(&cfg); err != nil {
		return nil, fmt.Errorf("parse config yaml: %w", err)
	}
	if err := Validate(&cfg); err != nil {
		return nil, err
	}

	return &cfg, nil
}

// Validate checks schema and semantic constraints.
func Validate(cfg *Config) error {
	if cfg == nil {
		return errors.New("configuration validation failed: root: is required")
	}

	issues := ValidationErrors{}
	add := func(field, message string) {
		issues.Issues = append(issues.Issues, ValidationError{Field: field, Message: message})
	}

	if cfg.Version != CurrentVersion {
		add("version", fmt.Sprintf("must be %d", CurrentVersion))
	}

	validateDefaults(&issues, cfg.Defaults)
	validateNotifications(&issues, cfg.Notifications)

	if cfg.Tasks == nil {
		add("tasks", "is required")
	} else {
		for name, task := range cfg.Tasks {
			validateTaskName(&issues, name)
			validateTask(&issues, name, task)
		}
	}

	if issues.HasIssues() {
		return issues
	}
	return nil
}

// ResolveTask returns a merged execution definition for a named task.
func (c *Config) ResolveTask(name string) (ResolvedTask, error) {
	task, ok := c.Tasks[name]
	if !ok {
		return ResolvedTask{}, fmt.Errorf("task %q not found", name)
	}

	timeout, err := resolveDuration(task.Timeout, c.Defaults.Timeout, 0)
	if err != nil {
		return ResolvedTask{}, fmt.Errorf("task %q timeout: %w", name, err)
	}
	retries := resolveRetries(task.Retries, c.Defaults.Retries, 0)
	retryBackoff, err := resolveDuration(task.RetryBackoff, c.Defaults.RetryBackoff, time.Second)
	if err != nil {
		return ResolvedTask{}, fmt.Errorf("task %q retry_backoff: %w", name, err)
	}
	notifyOn := resolveNotifyOn(task.NotifyOn, c.Defaults.NotifyOn, "failure")

	resolved := ResolvedTask{
		Name:         name,
		Source:       "task",
		Dir:          task.Dir,
		Env:          mapsClone(task.Env),
		Timeout:      timeout,
		Retries:      retries,
		RetryBackoff: retryBackoff,
		NotifyOn:     notifyOn,
	}

	if len(task.Exec) > 0 {
		resolved.UseShell = false
		resolved.Exec = slices.Clone(task.Exec)
		resolved.CommandPreview = joinCommandPreview(task.Exec)
	} else {
		resolved.UseShell = true
		resolved.Shell = task.Run
		resolved.CommandPreview = task.Run
	}

	return resolved, nil
}

// ResolveInline builds an execution definition for inline command mode.
func ResolveInline(args []string, name string, timeoutFlag string, retriesFlag int, notifyOnFlag string, defaults Defaults) (ResolvedTask, error) {
	if len(args) == 0 {
		return ResolvedTask{}, errors.New("inline command is required after --")
	}

	timeout, err := resolveDuration(timeoutFlag, defaults.Timeout, 0)
	if err != nil {
		return ResolvedTask{}, fmt.Errorf("inline timeout: %w", err)
	}
	retries := retriesFlag
	if retries == -1 {
		retries = resolveRetries(nil, defaults.Retries, 0)
	}
	if retries < 0 || retries > 10 {
		return ResolvedTask{}, errors.New("inline retries must be between 0 and 10")
	}
	retryBackoff, err := resolveDuration("", defaults.RetryBackoff, time.Second)
	if err != nil {
		return ResolvedTask{}, fmt.Errorf("inline retry_backoff: %w", err)
	}
	notifyOn := resolveNotifyOn(notifyOnFlag, defaults.NotifyOn, "failure")

	if name == "" {
		name = "inline"
	}

	return ResolvedTask{
		Name:           name,
		Source:         "inline",
		CommandPreview: joinCommandPreview(args),
		UseShell:       false,
		Exec:           slices.Clone(args),
		Timeout:        timeout,
		Retries:        retries,
		RetryBackoff:   retryBackoff,
		NotifyOn:       notifyOn,
	}, nil
}

// ResolveNotificationSettings returns validated provider configuration.
func (c *Config) ResolveNotificationSettings() (NotificationSettings, error) {
	desktop := true
	if c.Notifications.Desktop != nil {
		desktop = *c.Notifications.Desktop
	}

	webhookTimeout, err := resolveDuration(c.Notifications.WebhookTimeout, "", 5*time.Second)
	if err != nil {
		return NotificationSettings{}, fmt.Errorf("notifications.webhook_timeout: %w", err)
	}

	return NotificationSettings{
		DesktopEnabled: desktop,
		WebhookURL:     c.Notifications.WebhookURL,
		WebhookTimeout: webhookTimeout,
	}, nil
}

func validateDefaults(issues *ValidationErrors, d Defaults) {
	if d.Timeout != "" {
		if _, err := time.ParseDuration(d.Timeout); err != nil {
			issues.Issues = append(issues.Issues, ValidationError{Field: "defaults.timeout", Message: "must be a valid duration"})
		}
	}
	if d.Retries != nil && (*d.Retries < 0 || *d.Retries > 10) {
		issues.Issues = append(issues.Issues, ValidationError{Field: "defaults.retries", Message: "must be between 0 and 10"})
	}
	if d.RetryBackoff != "" {
		if _, err := time.ParseDuration(d.RetryBackoff); err != nil {
			issues.Issues = append(issues.Issues, ValidationError{Field: "defaults.retry_backoff", Message: "must be a valid duration"})
		}
	}
	if d.NotifyOn != "" && !slices.Contains(validNotifyOn, d.NotifyOn) {
		issues.Issues = append(issues.Issues, ValidationError{Field: "defaults.notify_on", Message: "must be one of never, failure, always"})
	}
}

func validateNotifications(issues *ValidationErrors, n Notifications) {
	if n.WebhookURL != "" {
		if _, err := url.ParseRequestURI(n.WebhookURL); err != nil {
			issues.Issues = append(issues.Issues, ValidationError{Field: "notifications.webhook_url", Message: "must be a valid URL"})
		}
	}
	if n.WebhookTimeout != "" {
		if _, err := time.ParseDuration(n.WebhookTimeout); err != nil {
			issues.Issues = append(issues.Issues, ValidationError{Field: "notifications.webhook_timeout", Message: "must be a valid duration"})
		}
	}
}

func validateTaskName(issues *ValidationErrors, name string) {
	if !taskNamePattern.MatchString(name) {
		issues.Issues = append(issues.Issues, ValidationError{
			Field:   fmt.Sprintf("tasks.%s", name),
			Message: "name must match ^[a-z0-9][a-z0-9_-]{0,62}$",
		})
	}
	if slices.Contains(reservedNames, name) {
		issues.Issues = append(issues.Issues, ValidationError{
			Field:   fmt.Sprintf("tasks.%s", name),
			Message: "name is reserved",
		})
	}
}

func validateTask(issues *ValidationErrors, name string, task Task) {
	field := fmt.Sprintf("tasks.%s", name)

	hasExec := len(task.Exec) > 0
	hasRun := task.Run != ""
	if hasExec == hasRun {
		issues.Issues = append(issues.Issues, ValidationError{
			Field:   field,
			Message: "must define exactly one of exec or run",
		})
	}

	if hasExec {
		for i, tok := range task.Exec {
			if tok == "" {
				issues.Issues = append(issues.Issues, ValidationError{
					Field:   fmt.Sprintf("%s.exec[%d]", field, i),
					Message: "must not be empty",
				})
			}
		}
	}

	if task.Timeout != "" {
		if _, err := time.ParseDuration(task.Timeout); err != nil {
			issues.Issues = append(issues.Issues, ValidationError{
				Field:   fmt.Sprintf("%s.timeout", field),
				Message: "must be a valid duration",
			})
		}
	}
	if task.Retries != nil && (*task.Retries < 0 || *task.Retries > 10) {
		issues.Issues = append(issues.Issues, ValidationError{
			Field:   fmt.Sprintf("%s.retries", field),
			Message: "must be between 0 and 10",
		})
	}
	if task.RetryBackoff != "" {
		if _, err := time.ParseDuration(task.RetryBackoff); err != nil {
			issues.Issues = append(issues.Issues, ValidationError{
				Field:   fmt.Sprintf("%s.retry_backoff", field),
				Message: "must be a valid duration",
			})
		}
	}
	if task.NotifyOn != "" && !slices.Contains(validNotifyOn, task.NotifyOn) {
		issues.Issues = append(issues.Issues, ValidationError{
			Field:   fmt.Sprintf("%s.notify_on", field),
			Message: "must be one of never, failure, always",
		})
	}
}

func resolveDuration(primary, fallback string, defaultValue time.Duration) (time.Duration, error) {
	value := primary
	if value == "" {
		value = fallback
	}
	if value == "" {
		return defaultValue, nil
	}
	d, err := time.ParseDuration(value)
	if err != nil {
		return 0, errors.New("must be a valid duration")
	}
	return d, nil
}

func resolveRetries(primary, fallback *int, defaultValue int) int {
	if primary != nil {
		return *primary
	}
	if fallback != nil {
		return *fallback
	}
	return defaultValue
}

func resolveNotifyOn(primary, fallback, defaultValue string) string {
	value := primary
	if value == "" {
		value = fallback
	}
	if value == "" {
		return defaultValue
	}
	return value
}

func joinCommandPreview(args []string) string {
	if len(args) == 0 {
		return ""
	}
	return strings.Join(args, " ")
}

func mapsClone(m map[string]string) map[string]string {
	if len(m) == 0 {
		return nil
	}
	out := make(map[string]string, len(m))
	for k, v := range m {
		out[k] = v
	}
	return out
}

package cli

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"os"
	"time"

	"github.com/spf13/cobra"

	"github.com/mcmanussliam/otto/internal/config"
	"github.com/mcmanussliam/otto/internal/history"
	"github.com/mcmanussliam/otto/internal/model"
	"github.com/mcmanussliam/otto/internal/notify"
	"github.com/mcmanussliam/otto/internal/output"
	"github.com/mcmanussliam/otto/internal/runner"
)

func newRunCommand(app *application) *cobra.Command {
	var configPath string
	var inlineName string
	var inlineTimeout string
	var inlineRetries int
	var inlineNotifyOn string

	cmd := &cobra.Command{
		Use:   "run [task]",
		Short: "Run a named task or an inline command",
		Long: "Run a named task from config:\n" +
			"  otto run test\n\n" +
			"Run an inline command:\n" +
			"  otto run -- go test ./...\n",
		Args: cobra.ArbitraryArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			ctx, cancel := app.context()
			defer cancel()

			dashAt := cmd.ArgsLenAtDash()
			if dashAt >= 0 {
				if dashAt != 0 {
					return maybeUsageError(errors.New("inline mode requires only command args after --"))
				}

				resolved, notifications, err := resolveInlineRun(cmd, args, configPath, inlineName, inlineTimeout, inlineRetries, inlineNotifyOn)
				if err != nil {
					return err
				}

				return app.executeRun(ctx, cmd, resolved, notifications)
			}

			if cmd.Flags().Changed("name") || cmd.Flags().Changed("timeout") || cmd.Flags().Changed("retries") || cmd.Flags().Changed("notify-on") {
				return maybeUsageError(errors.New("--name, --timeout, --retries, and --notify-on are inline-only flags; use with 'otto run -- <command>'"))
			}

			if len(args) != 1 {
				return maybeUsageError(errors.New("named task mode requires exactly one task name"))
			}

			taskName := args[0]

			cfg, err := config.Load(configPath)
			if err != nil {
				return classifyConfigError(err)
			}

			resolved, err := cfg.ResolveTask(taskName)
			if err != nil {
				return maybeUsageError(err)
			}

			notifications, err := cfg.ResolveNotificationSettings()
			if err != nil {
				return maybeUsageError(err)
			}

			return app.executeRun(ctx, cmd, resolved, notifications)
		},
	}

	cmd.Flags().StringVar(&configPath, "config", defaultConfigPath, "Path to config file")
	cmd.Flags().StringVar(&inlineName, "name", "", "Inline run name (inline mode only)")
	cmd.Flags().StringVar(&inlineTimeout, "timeout", "", "Inline timeout duration (inline mode only)")
	cmd.Flags().IntVar(&inlineRetries, "retries", -1, "Inline retry count (inline mode only)")
	cmd.Flags().StringVar(&inlineNotifyOn, "notify-on", "", "Inline notification policy: never|failure|always (inline mode only)")

	return cmd
}

func resolveInlineRun(
	cmd *cobra.Command,
	args []string,
	configPath string,
	inlineName string,
	inlineTimeout string,
	inlineRetries int,
	inlineNotifyOn string,
) (config.ResolvedTask, config.NotificationSettings, error) {
	cfg, err := maybeLoadConfigForInline(configPath, cmd.Flags().Changed("config"))
	if err != nil {
		return config.ResolvedTask{}, config.NotificationSettings{}, err
	}

	defaults := config.Defaults{}
	notifications := config.NotificationSettings{
		DesktopEnabled: true,
		WebhookTimeout: 5 * time.Second,
	}

	if cfg != nil {
		defaults = cfg.Defaults
		notifications, err = cfg.ResolveNotificationSettings()
		if err != nil {
			return config.ResolvedTask{}, config.NotificationSettings{}, maybeUsageError(err)
		}
	}

	resolved, err := config.ResolveInline(args, inlineName, inlineTimeout, inlineRetries, inlineNotifyOn, defaults)
	if err != nil {
		return config.ResolvedTask{}, config.NotificationSettings{}, maybeUsageError(err)
	}

	return resolved, notifications, nil
}

func maybeLoadConfigForInline(path string, explicit bool) (*config.Config, error) {
	if !fileExists(path) {
		if explicit {
			return nil, maybeUsageError(fmt.Errorf("config file %s not found", path))
		}
		return nil, nil
	}

	cfg, err := config.Load(path)
	if err != nil {
		return nil, classifyConfigError(err)
	}
	return cfg, nil
}

func (a *application) executeRun(ctx context.Context, cmd *cobra.Command, resolved config.ResolvedTask, notifications config.NotificationSettings) error {
	req := runner.Request{
		Name:           resolved.Name,
		Source:         model.RunSource(resolved.Source),
		CommandPreview: resolved.CommandPreview,
		UseShell:       resolved.UseShell,
		Exec:           resolved.Exec,
		Shell:          resolved.Shell,
		Dir:            resolved.Dir,
		Env:            resolved.Env,
		Timeout:        resolved.Timeout,
		Retries:        resolved.Retries,
		RetryBackoff:   resolved.RetryBackoff,
	}

	result, runErr := runner.Execute(ctx, req)
	rec := model.RunRecord{
		ID:             newRecordID(),
		Name:           resolved.Name,
		Source:         model.RunSource(resolved.Source),
		CommandPreview: resolved.CommandPreview,
		StartedAt:      result.StartedAt,
		DurationMs:     result.Duration.Milliseconds(),
		ExitCode:       result.ExitCode,
		Status:         result.Status,
		StderrTail:     result.StderrTail,
	}

	store := history.NewStore(a.historyPath)
	if err := store.Append(ctx, rec); err != nil {
		return maybeInternalError(err)
	}

	if shouldNotify(resolved.NotifyOn, result.Status) {
		manager := notify.Manager{
			DesktopEnabled: notifications.DesktopEnabled,
			WebhookURL:     notifications.WebhookURL,
			WebhookTimeout: notifications.WebhookTimeout,
		}
		err := manager.Notify(ctx, notify.Event{
			Name:           rec.Name,
			Source:         string(rec.Source),
			Status:         string(rec.Status),
			ExitCode:       rec.ExitCode,
			Duration:       result.Duration,
			StartedAt:      rec.StartedAt,
			CommandPreview: rec.CommandPreview,
			StderrTail:     rec.StderrTail,
		})

		if err != nil {
			_, _ = fmt.Fprintf(cmd.ErrOrStderr(), "%s failed to send notification: %v\n", output.Warning("⚠"), err)
		}
	}

	if runErr != nil {
		return maybeRuntimeError(runErr)
	}

	_, _ = fmt.Fprintf(
		cmd.OutOrStdout(),
		"%s run %q finished in %s\n",
		output.Success("✔"),
		rec.Name,
		output.Accent(result.Duration.Round(time.Millisecond).String()),
	)
	return nil
}

func classifyConfigError(err error) error {
	var validationErr config.ValidationErrors
	if errors.As(err, &validationErr) {
		return maybeUsageError(validationErr)
	}

	if errors.Is(err, os.ErrNotExist) {
		return maybeUsageError(err)
	}

	return maybeInternalError(err)
}

func shouldNotify(policy string, status model.RunStatus) bool {
	switch policy {
	case "never":
		return false
	case "always":
		return true
	default:
		return status == model.StatusFailed
	}
}

func newRecordID() string {
	var buf [8]byte
	if _, err := rand.Read(buf[:]); err != nil {
		return fmt.Sprintf("%d", time.Now().UnixNano())
	}

	return fmt.Sprintf("%d-%s", time.Now().UnixMilli(), hex.EncodeToString(buf[:]))
}

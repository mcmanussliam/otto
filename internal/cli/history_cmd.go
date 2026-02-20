package cli

import (
	"context"
	"errors"

	"github.com/spf13/cobra"

	"github.com/mcmanussliam/otto/internal/history"
	"github.com/mcmanussliam/otto/internal/output"
)

func newHistoryCommand(app *application) *cobra.Command {
	var limit int
	var status string
	var source string

	cmd := &cobra.Command{
		Use:   "history",
		Short: "Show recent run history",
		Args:  cobra.NoArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			if status != "" && status != "success" && status != "failed" {
				return maybeUsageError(errors.New("--status must be success or failed"))
			}

			if source != "" && source != "task" && source != "inline" {
				return maybeUsageError(errors.New("--source must be task or inline"))
			}

			store := history.NewStore(app.historyPath)
			rows, err := store.List(context.Background(), history.Filter{
				Limit:  limit,
				Status: status,
				Source: source,
			})

			if err != nil {
				return maybeInternalError(err)
			}

			output.PrintHistory(cmd.OutOrStdout(), rows)
			return nil
		},
	}

	cmd.Flags().IntVar(&limit, "limit", 20, "Max records to show")
	cmd.Flags().StringVar(&status, "status", "", "Filter by status: success|failed")
	cmd.Flags().StringVar(&source, "source", "", "Filter by source: task|inline")
	return cmd
}

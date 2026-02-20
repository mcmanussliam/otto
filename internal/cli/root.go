package cli

import (
	"context"
	"os"
	"time"

	"github.com/spf13/cobra"

	"github.com/mcmanussliam/otto/internal/history"
	"github.com/mcmanussliam/otto/internal/output"
)

// NewRootCommand builds the root otto command tree.
func NewRootCommand() *cobra.Command {
	app := &application{
		historyPath: history.DefaultPath,
	}
	var noColor bool

	root := &cobra.Command{
		Use:           "otto",
		Short:         "Task runner with run history and notifications",
		SilenceErrors: true,
		SilenceUsage:  true,
		PersistentPreRun: func(cmd *cobra.Command, args []string) {
			output.Configure(noColor)
		},
	}

	root.PersistentFlags().BoolVar(&noColor, "no-color", false, "Disable ANSI colors")
	installHelp(root, &noColor)

	root.AddCommand(newInitCommand(app))
	root.AddCommand(newRunCommand(app))
	root.AddCommand(newHistoryCommand(app))
	root.AddCommand(newVersionCommand())

	return root
}

type application struct {
	historyPath string
}

func (a *application) context() (context.Context, context.CancelFunc) {
	return context.WithTimeout(context.Background(), 24*time.Hour)
}

func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func maybeUsageError(err error) error {
	return WithCode(ExitUsage, err)
}

func maybeInternalError(err error) error {
	return WithCode(ExitInternal, err)
}

func maybeRuntimeError(err error) error {
	return WithCode(ExitRuntimeFailure, err)
}

package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/mcmanussliam/otto/internal/version"
)

func newVersionCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "version",
		Short: "Print otto version",
		Run: func(cmd *cobra.Command, args []string) {
			_, _ = fmt.Fprintln(cmd.OutOrStdout(), version.Value)
		},
	}
}

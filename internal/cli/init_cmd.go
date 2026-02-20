package cli

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
)

const defaultConfigPath = "./otto.yml"

const defaultConfigTemplate = `version: 1
defaults:
  timeout: "2m"
  retries: 0
  retry_backoff: "1s"
  notify_on: failure
notifications:
  desktop: true
tasks:
  test:
    description: run unit tests
    exec: ["go", "test", "./..."]
`

func newInitCommand(app *application) *cobra.Command {
	var configPath string
	var force bool

	cmd := &cobra.Command{
		Use:   "init",
		Short: "Create a starter otto.yml config",
		RunE: func(cmd *cobra.Command, args []string) error {
			if !force && fileExists(configPath) {
				return maybeUsageError(fmt.Errorf("%s already exists (use --force to overwrite)", configPath))
			}

			if err := os.WriteFile(configPath, []byte(defaultConfigTemplate), 0o644); err != nil {
				return maybeInternalError(fmt.Errorf("write %s: %w", configPath, err))
			}

			_, _ = fmt.Fprintf(cmd.OutOrStdout(), "created %s\n", configPath)
			return nil
		},
	}

	cmd.Flags().StringVar(&configPath, "config", defaultConfigPath, "Path to config file")
	cmd.Flags().BoolVar(&force, "force", false, "Overwrite existing file")
	return cmd
}

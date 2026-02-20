package cli

import (
	"fmt"
	"io"
	"slices"
	"strings"
	"unicode/utf8"

	"github.com/spf13/cobra"

	"github.com/mcmanussliam/otto/internal/output"
)

func installHelp(root *cobra.Command, noColor *bool) {
	root.SetHelpFunc(func(cmd *cobra.Command, args []string) {
		if noColor != nil {
			output.Configure(*noColor)
		}
		printHelp(cmd.OutOrStdout(), cmd)
	})
}

func printHelp(w io.Writer, cmd *cobra.Command) {
	if text := strings.TrimSpace(cmd.Long); text != "" {
		_, _ = fmt.Fprintln(w, text)
	} else if text := strings.TrimSpace(cmd.Short); text != "" {
		_, _ = fmt.Fprintln(w, text)
	}

	usage := cmd.UseLine()
	if !cmd.Runnable() && cmd.HasAvailableSubCommands() {
		usage = fmt.Sprintf("%s [command]", cmd.CommandPath())
	}
	section(w, "Usage")
	_, _ = fmt.Fprintf(w, "  %s\n", usage)

	children := availableCommands(cmd)
	if len(children) > 0 {
		section(w, "Commands")

		nameW := utf8.RuneCountInString("help")
		for _, child := range children {
			nameW = max(nameW, utf8.RuneCountInString(child.Name()))
		}

		for _, child := range children {
			_, _ = fmt.Fprintf(w, "  %s  %s\n", padRight(child.Name(), nameW), child.Short)
		}
	}

	if cmd.HasAvailableLocalFlags() {
		section(w, "Flags")
		_, _ = fmt.Fprint(w, cmd.LocalFlags().FlagUsages())
	}

	if cmd.HasAvailableInheritedFlags() {
		section(w, "Global Flags")
		_, _ = fmt.Fprint(w, cmd.InheritedFlags().FlagUsages())
	}

	if text := strings.TrimSpace(cmd.Example); text != "" {
		section(w, "Examples")
		_, _ = fmt.Fprintln(w, text)
	}

	if len(children) > 0 {
		_, _ = fmt.Fprintf(w, "\n%s\n", output.Muted(fmt.Sprintf(`Run "%s [command] --help" for more information.`, cmd.CommandPath())))
	}
}

func section(w io.Writer, title string) {
	_, _ = fmt.Fprintf(w, "\n%s\n", output.Bold(output.Accent(title)))
}

func availableCommands(cmd *cobra.Command) []*cobra.Command {
	out := make([]*cobra.Command, 0, len(cmd.Commands()))
	for _, child := range cmd.Commands() {
		if !child.IsAvailableCommand() || child.IsAdditionalHelpTopicCommand() {
			continue
		}
		out = append(out, child)
	}

	slices.SortFunc(out, func(a, b *cobra.Command) int {
		return strings.Compare(a.Name(), b.Name())
	})
	return out
}

func padRight(value string, width int) string {
	diff := width - utf8.RuneCountInString(value)
	if diff <= 0 {
		return value
	}
	return value + strings.Repeat(" ", diff)
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}

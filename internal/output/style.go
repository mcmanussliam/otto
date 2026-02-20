package output

import (
	"fmt"
	"os"
	"strings"
	"sync/atomic"
)

var colorsEnabled atomic.Bool

func init() {
	Configure(false)
}

// Configure updates terminal styling behavior from flags and environment.
func Configure(noColor bool) {
	enabled := !noColor

	if os.Getenv("NO_COLOR") != "" {
		enabled = false
	}

	if strings.EqualFold(os.Getenv("TERM"), "dumb") {
		enabled = false
	}

	if os.Getenv("CLICOLOR_FORCE") == "1" {
		enabled = true
	}

	colorsEnabled.Store(enabled)
}

// SetColorEnabled force-enables or disables ANSI styling.
func SetColorEnabled(enabled bool) {
	colorsEnabled.Store(enabled)
}

func style(code, text string) string {
	if text == "" || !colorsEnabled.Load() {
		return text
	}

	return fmt.Sprintf("\x1b[%sm%s\x1b[0m", code, text)
}

// Bold makes text bold.
func Bold(text string) string {
	return style("1", text)
}

// Muted dims text.
func Muted(text string) string {
	return style("2", text)
}

// Accent highlights text with cyan.
func Accent(text string) string {
	return style("36", text)
}

// Success highlights successful text.
func Success(text string) string {
	return style("32;1", text)
}

// Failure highlights failed text.
func Failure(text string) string {
	return style("31;1", text)
}

// Warning highlights warning text.
func Warning(text string) string {
	return style("33;1", text)
}

// Info highlights informational text.
func Info(text string) string {
	return style("34;1", text)
}

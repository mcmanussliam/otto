package main

import (
	"fmt"
	"os"

	"github.com/mcmanussliam/otto/internal/cli"
)

func main() {
	root := cli.NewRootCommand()
	if err := root.Execute(); err != nil {
		_, _ = fmt.Fprintln(os.Stderr, err)
		os.Exit(cli.CodeFromError(err))
	}
}

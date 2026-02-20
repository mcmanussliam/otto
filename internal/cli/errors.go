package cli

import (
	"errors"
	"fmt"
)

const (
	// ExitSuccess indicates successful command completion.
	ExitSuccess = 0
	// ExitRuntimeFailure indicates task execution failure.
	ExitRuntimeFailure = 1
	// ExitUsage indicates command usage or validation failure.
	ExitUsage = 2
	// ExitInternal indicates unexpected internal failure.
	ExitInternal = 3
)

// ExitError carries a process exit code with an error.
type ExitError struct {
	Code int
	Err  error
}

// Error implements error.
func (e ExitError) Error() string {
	if e.Err == nil {
		return fmt.Sprintf("exit code %d", e.Code)
	}

	return e.Err.Error()
}

// Unwrap returns the wrapped error.
func (e ExitError) Unwrap() error {
	return e.Err
}

// WithCode wraps err with a process exit code.
func WithCode(code int, err error) error {
	if err == nil {
		return nil
	}

	return ExitError{Code: code, Err: err}
}

// CodeFromError extracts an exit code from err.
func CodeFromError(err error) int {
	if err == nil {
		return ExitSuccess
	}

	var ee ExitError
	if ok := errors.As(err, &ee); ok {
		return ee.Code
	}

	return ExitInternal
}

package report

import (
	"errors"
	"io"
	"testing"
)

type failedWriter struct{}

func (failedWriter) Write([]byte) (int, error) { return 0, io.ErrClosedPipe }

func TestHumanOutputPropagatesWriterFailure(t *testing.T) {
	if err := Write(failedWriter{}, Result{}, false); !errors.Is(err, io.ErrClosedPipe) {
		t.Fatalf("expected the writer error, got %v", err)
	}
}

package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"maintainablecheck/internal/report"
)

func TestCLISelectionAndNativeCheckExitCodes(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "data.json"), []byte("{"), 0o600); err != nil {
		t.Fatal(err)
	}
	var output, errors bytes.Buffer
	code, err := run([]string{"select", "--root", root, "--json"}, &output, &errors)
	var selected report.Result
	if decodeErr := json.Unmarshal(output.Bytes(), &selected); decodeErr != nil {
		t.Fatal(decodeErr, output.String())
	}
	if code != 0 || err != nil || selected.Mode != "list" || selected.Summary.Selected != 1 || selected.Summary.Checked != 0 {
		t.Fatalf("%d %v %v", code, err, selected)
	}
	output.Reset()
	code, err = run([]string{"check", "--root", root, "--checks", "json", "--json"}, &output, &errors)
	if code != 1 || err != nil {
		t.Fatalf("expected syntax-error exit, got %d %v", code, err)
	}
	if _, err = run([]string{"check", "--root", root, "--checks", "llm"}, &output, &errors); err == nil {
		t.Fatal("expected an unavailable-check error")
	}
}

func TestCLIConfigurationErrorsNameTheirSource(t *testing.T) {
	root := t.TempDir()
	name := filepath.Join(root, "checkignore.json")
	if err := os.WriteFile(name, []byte("null"), 0o600); err != nil {
		t.Fatal(err)
	}
	_, err := run([]string{"select", "--root", root}, &bytes.Buffer{}, &bytes.Buffer{})
	if err == nil || !bytes.Contains([]byte(err.Error()), []byte("checkignore.json")) {
		t.Fatal(err)
	}
}

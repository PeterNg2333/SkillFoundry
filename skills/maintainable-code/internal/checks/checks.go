package checks

import (
	"fmt"
	"os"
	"path/filepath"
	"slices"

	"maintainablecheck/internal/config"
	"maintainablecheck/internal/report"
	"maintainablecheck/internal/selection"
)

type Capability struct {
	ID        string   `json:"id"`
	Languages []string `json:"languages"`
	Purpose   string   `json:"purpose"`
}

func Capabilities() []Capability {
	return []Capability{
		{ID: "json", Languages: []string{"json"}, Purpose: "Strict JSON syntax"},
		{ID: "dependencies", Languages: []string{"package.json", "go.mod"}, Purpose: "Declared dependency counts and configured limits"},
		{ID: "go", Languages: []string{"go"}, Purpose: "Go syntax, gofmt, imports and function AST counts"},
		{ID: "comments", Languages: []string{"go"}, Purpose: "AST comment counts and optional presence rule"},
	}
}

func Validate(names []string) error {
	seen := map[string]bool{}
	for _, name := range names {
		if seen[name] {
			return fmt.Errorf("duplicate native check %q", name)
		}
		seen[name] = true
		found := false
		for _, capability := range Capabilities() {
			found = found || name == capability.ID
		}
		if !found {
			return fmt.Errorf("unknown native check %q", name)
		}
	}
	return nil
}

func Run(root string, settings config.Resolved, files []selection.File, names []string) (report.Result, error) {
	result := report.Result{Diagnostics: []report.Finding{}, Metrics: []report.Metric{}, Checks: []report.CheckStatus{}}
	covered := map[string]bool{}
	counts := map[string]int{}
	for _, file := range files {
		isManifest := filepath.Base(file.Path) == "package.json" || filepath.Base(file.Path) == "go.mod"
		jsonSelected := file.Type == "json" && slices.Contains(names, "json")
		dependenciesSelected := isManifest && slices.Contains(names, "dependencies")
		goSelected := file.Type == "go" && slices.Contains(names, "go")
		commentsSelected := file.Type == "go" && slices.Contains(names, "comments")
		if !jsonSelected && !dependenciesSelected && !goSelected && !commentsSelected {
			continue
		}
		source, err := os.ReadFile(filepath.Join(root, filepath.FromSlash(file.Path)))
		if err != nil {
			return result, fmt.Errorf("read %s: %w", file.Path, err)
		}
		covered[file.Path] = true
		if jsonSelected {
			result.Diagnostics = append(result.Diagnostics, checkJSON(file.Path, source)...)
			counts["json"]++
		}
		if dependenciesSelected {
			findings, metrics := checkDependencies(file.Path, source, settings.Limits.DirectDependencies)
			result.Diagnostics = append(result.Diagnostics, findings...)
			result.Metrics = append(result.Metrics, metrics...)
			counts["dependencies"]++
		}
		if goSelected || commentsSelected {
			findings, metrics := checkGo(file.Path, source, settings.Limits, goSelected, commentsSelected)
			result.Diagnostics = append(result.Diagnostics, findings...)
			result.Metrics = append(result.Metrics, metrics...)
			if goSelected {
				counts["go"]++
			}
			if commentsSelected {
				counts["comments"]++
			}
		}
	}
	for _, name := range names {
		status := "checked"
		if counts[name] == 0 {
			status = "not-applicable"
		}
		result.Checks = append(result.Checks, report.CheckStatus{ID: name, Status: status, Files: counts[name]})
	}
	result.Summary.Checked = len(covered)
	result.Summary.Unchecked = len(files) - len(covered)
	result.CountFindings()
	return result, nil
}

func finding(file, severity, rule, message string, line, column int) report.Finding {
	return report.Finding{File: file, Line: line, Column: column, Severity: severity, Rule: rule, Message: message, Origin: "static"}
}

func metric(file, check, name string, value int, unit string) report.Metric {
	return report.Metric{File: file, Check: check, Name: name, Value: value, Unit: unit}
}

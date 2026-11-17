package report

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"

	"maintainablecheck/internal/config"
	"maintainablecheck/internal/selection"
)

type Finding struct {
	File     string `json:"file"`
	Line     int    `json:"line"`
	Column   int    `json:"column"`
	Severity string `json:"severity"`
	Rule     string `json:"rule"`
	Message  string `json:"message"`
	Origin   string `json:"origin"`
}

type Metric struct {
	File  string `json:"file"`
	Check string `json:"check"`
	Name  string `json:"name"`
	Value int    `json:"value"`
	Unit  string `json:"unit"`
}

type CheckStatus struct {
	ID     string `json:"id"`
	Status string `json:"status"`
	Files  int    `json:"files"`
}

type Summary struct {
	Selected  int `json:"selected"`
	Checked   int `json:"checked"`
	Unchecked int `json:"unchecked"`
	Skipped   int `json:"skipped"`
	Errors    int `json:"errors"`
	Warnings  int `json:"warnings"`
}

type ConfigSource struct {
	Defaults string  `json:"defaults"`
	Project  *string `json:"project"`
}

type Result struct {
	SchemaVersion int                 `json:"schemaVersion"`
	Root          string              `json:"root"`
	Mode          string              `json:"mode"`
	Config        ConfigSource        `json:"config"`
	Configuration *config.Resolved    `json:"configuration,omitempty"`
	Files         []selection.File    `json:"files"`
	Skipped       []selection.Skipped `json:"skipped"`
	Diagnostics   []Finding           `json:"diagnostics"`
	Metrics       []Metric            `json:"metrics"`
	Checks        []CheckStatus       `json:"checks"`
	Summary       Summary             `json:"summary"`
}

func (r *Result) CountFindings() {
	r.Summary.Errors = 0
	r.Summary.Warnings = 0
	for _, finding := range r.Diagnostics {
		switch finding.Severity {
		case "error":
			r.Summary.Errors++
		case "warning":
			r.Summary.Warnings++
		}
	}
}

func Write(output io.Writer, result Result, machine bool) error {
	if machine {
		encoder := json.NewEncoder(output)
		encoder.SetIndent("", "  ")
		return encoder.Encode(result)
	}
	writer := bufio.NewWriter(output)
	if result.Mode == "list" {
		for _, file := range result.Files {
			fmt.Fprintf(writer, "%s %s\n", file.Type, file.Path)
		}
	}
	for _, finding := range result.Diagnostics {
		fmt.Fprintf(writer, "%s:%d:%d %s %s %s\n", finding.File, finding.Line, finding.Column, finding.Severity, finding.Rule, finding.Message)
	}
	for _, metric := range result.Metrics {
		fmt.Fprintf(writer, "%s %s %d %s\n", metric.File, metric.Name, metric.Value, metric.Unit)
	}
	fmt.Fprintf(writer, "%d selected, %d checked, %d unchecked, %d paths skipped, %d errors, %d warnings\n", result.Summary.Selected, result.Summary.Checked, result.Summary.Unchecked, result.Summary.Skipped, result.Summary.Errors, result.Summary.Warnings)
	return writer.Flush()
}

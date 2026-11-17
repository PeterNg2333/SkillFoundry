package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime/debug"
	"strings"

	"maintainablecheck/internal/checks"
	"maintainablecheck/internal/config"
	"maintainablecheck/internal/report"
	"maintainablecheck/internal/selection"
)

func main() {
	code, err := run(os.Args[1:], os.Stdout, os.Stderr)
	if err != nil {
		fmt.Fprintln(os.Stderr, "Static check failed.", err)
		code = 2
	}
	os.Exit(code)
}

func run(arguments []string, output, errors io.Writer) (int, error) {
	command := "check"
	if len(arguments) > 0 && !strings.HasPrefix(arguments[0], "-") {
		command, arguments = arguments[0], arguments[1:]
	}
	if command != "check" && command != "select" && command != "capabilities" {
		return 2, fmt.Errorf("unknown command %q", command)
	}
	options := flag.NewFlagSet("maintainable-check "+command, flag.ContinueOnError)
	options.SetOutput(errors)
	root := options.String("root", ".", "Project root")
	configuration := options.String("config", "", "Project checkignore configuration")
	machine := options.Bool("json", false, "Machine-readable report")
	names := options.String("checks", "json,dependencies,go", "Comma-separated native check IDs")
	if err := options.Parse(arguments); err != nil {
		if err == flag.ErrHelp {
			return 0, nil
		}
		return 2, err
	}
	if command == "capabilities" {
		information, _ := debug.ReadBuildInfo()
		return 0, json.NewEncoder(output).Encode(struct {
			Checks             []checks.Capability `json:"checks"`
			Runtime            string              `json:"runtime"`
			ModuleDependencies int                 `json:"moduleDependencies"`
		}{checks.Capabilities(), information.GoVersion, len(information.Deps)})
	}
	absolute, err := filepath.Abs(*root)
	if err != nil {
		return 2, err
	}
	absolute, err = filepath.EvalSymlinks(absolute)
	if err != nil {
		return 2, err
	}
	settings, err := config.Load(absolute, *configuration)
	if err != nil {
		return 2, err
	}
	selection, err := selection.Select(absolute, options.Args(), settings)
	if err != nil {
		return 2, err
	}
	result := report.Result{Root: absolute, Mode: "list", Config: report.ConfigSource{Defaults: settings.Defaults, Project: settings.Project}, Configuration: &settings, Files: selection.Files, Skipped: selection.Skipped, Diagnostics: []report.Finding{}, Metrics: []report.Metric{}, Checks: []report.CheckStatus{}}
	if command == "check" {
		selected := strings.Split(*names, ",")
		if err := checks.Validate(selected); err != nil {
			return 2, err
		}
		result, err = checks.Run(absolute, settings, selection.Files, selected)
		if err != nil {
			return 2, err
		}
		result.Root, result.Mode = absolute, "check"
		result.Config = report.ConfigSource{Defaults: settings.Defaults, Project: settings.Project}
		result.Files, result.Skipped = selection.Files, selection.Skipped
	}
	result.Summary.Selected, result.Summary.Skipped = len(selection.Files), len(selection.Skipped)
	result.SchemaVersion = 1
	if err := report.Write(output, result, *machine); err != nil {
		return 2, err
	}
	if result.Summary.Errors > 0 {
		return 1, nil
	}
	return 0, nil
}

package checks

import (
	"bytes"
	"fmt"
	"go/ast"
	"go/format"
	"go/parser"
	"go/scanner"
	"go/token"

	"maintainablecheck/internal/config"
	"maintainablecheck/internal/report"
)

func checkGo(file string, source []byte, limits config.Limits, syntax, comments bool) ([]report.Finding, []report.Metric) {
	positions := token.NewFileSet()
	tree, err := parser.ParseFile(positions, file, source, parser.ParseComments|parser.AllErrors)
	if err != nil {
		return goSyntaxErrors(file, err), nil
	}
	findings := []report.Finding{}
	metrics := []report.Metric{}
	if syntax {
		functions := 0
		ast.Inspect(tree, func(node ast.Node) bool {
			if _, ok := node.(*ast.FuncDecl); ok {
				functions++
			}
			return true
		})
		metrics = append(metrics, metric(file, "go", "imports", len(tree.Imports), "imports"), metric(file, "go", "functionDeclarations", functions, "functions"))
		formatted, err := format.Source(source)
		if err != nil {
			return goSyntaxErrors(file, err), metrics
		}
		if !bytes.Equal(formatted, source) {
			findings = append(findings, finding(file, "error", "go/format", "File differs from gofmt output.", 1, 1))
		}
		if limits.ImportsPerFile != nil && len(tree.Imports) > *limits.ImportsPerFile {
			message := fmt.Sprintf("Imports %d exceed configured limit %d.", len(tree.Imports), *limits.ImportsPerFile)
			findings = append(findings, finding(file, "warning", "go/import-count", message, 1, 1))
		}
	}
	if comments {
		metrics = append(metrics, metric(file, "comments", "commentGroups", len(tree.Comments), "groups"))
		if limits.RequireComments && len(tree.Comments) == 0 {
			findings = append(findings, finding(file, "warning", "comments/presence", "File has no AST comment groups.", 1, 1))
		}
	}
	return findings, metrics
}

func goSyntaxErrors(file string, err error) []report.Finding {
	if errors, ok := err.(scanner.ErrorList); ok {
		findings := make([]report.Finding, 0, len(errors))
		for _, problem := range errors {
			findings = append(findings, finding(file, "error", "go/syntax", problem.Msg, problem.Pos.Line, problem.Pos.Column))
		}
		return findings
	}
	return []report.Finding{finding(file, "error", "go/syntax", err.Error(), 1, 1)}
}

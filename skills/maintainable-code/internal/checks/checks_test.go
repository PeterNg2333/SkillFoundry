package checks

import (
	"go/format"
	"os"
	"path/filepath"
	"testing"

	"maintainablecheck/internal/config"
	"maintainablecheck/internal/report"
	"maintainablecheck/internal/selection"
)

func value(t *testing.T, metrics []report.Metric, name string) int {
	t.Helper()
	for _, metric := range metrics {
		if metric.Name == name {
			return metric.Value
		}
	}
	t.Fatalf("metric %s was not produced", name)
	return 0
}

func TestDependencyCountsDeduplicateAcrossDeclaredGroups(t *testing.T) {
	limit := 3
	source := []byte(`{"dependencies":{"a":"1","b":"1"},"devDependencies":{"a":"1","c":"1"},"peerDependencies":{"d":"1"},"optionalDependencies":{"b":"1"}}`)
	findings, metrics := checkDependencies("package.json", source, &limit)
	if value(t, metrics, "runtimeDependencies") != 2 || value(t, metrics, "developmentDependencies") != 2 || value(t, metrics, "uniqueDeclaredDependencies") != 4 {
		t.Fatal(metrics)
	}
	if len(findings) != 1 || findings[0].Rule != "dependencies/count" {
		t.Fatal(findings)
	}
}

func TestGoModuleCountsDistinguishDirectAndIndirect(t *testing.T) {
	source := []byte("module example.com/test\n\ngo 1.23.0\n\nrequire (\n example.com/direct v1.0.0\n example.com/indirect v1.0.0 // indirect\n)\n")
	findings, metrics := checkDependencies("go.mod", source, nil)
	if len(findings) != 0 || value(t, metrics, "directRequiredModules") != 1 || value(t, metrics, "indirectRequiredModules") != 1 {
		t.Fatalf("%v %v", findings, metrics)
	}
}

func TestGoASTCommentsIgnoreCommentLikeStrings(t *testing.T) {
	source, err := format.Source([]byte("package sample\nimport \"fmt\"\n// Explains the side effect.\nfunc Print() { fmt.Println(\"// string\") }\n"))
	if err != nil {
		t.Fatal(err)
	}
	findings, metrics := checkGo("sample.go", source, config.Limits{}, true, true)
	if len(findings) != 0 || value(t, metrics, "imports") != 1 || value(t, metrics, "functionDeclarations") != 1 || value(t, metrics, "commentGroups") != 1 {
		t.Fatalf("%v %v", findings, metrics)
	}
	findings, metrics = checkGo("sample.go", []byte("package sample\nconst Text = \"// string\"\n"), config.Limits{RequireComments: true}, false, true)
	if value(t, metrics, "commentGroups") != 0 || len(findings) != 1 || findings[0].Rule != "comments/presence" {
		t.Fatalf("%v %v", findings, metrics)
	}
}

func TestGoFormatIsAnEnforcedStyleCheck(t *testing.T) {
	findings, _ := checkGo("sample.go", []byte("package sample\nfunc Value( ){ }\n"), config.Limits{}, true, false)
	if len(findings) != 1 || findings[0].Rule != "go/format" || findings[0].Severity != "error" {
		t.Fatal(findings)
	}
}

func TestJSONStrictSyntaxAndBOMPositions(t *testing.T) {
	if findings := checkJSON("large.json", []byte(`{"large":1e400}`)); len(findings) != 0 {
		t.Fatal(findings)
	}
	if findings := checkJSON("valid.json", []byte("\ufeff{\"enabled\":true}")); len(findings) != 0 {
		t.Fatal(findings)
	}
	findings := checkJSON("broken.json", []byte("\ufeff{\n\"enabled\":\n}"))
	if len(findings) != 1 || findings[0].Line != 3 || findings[0].Column != 1 {
		t.Fatal(findings)
	}
	for _, source := range []string{`{/* comment */"enabled":true}`, `{"enabled":true,}`, ""} {
		if len(checkJSON("broken.json", []byte(source))) == 0 {
			t.Fatalf("expected strict JSON rejection for %s", source)
		}
	}
}

func TestPartialChecksReportActualCoverage(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "package.json"), []byte(`{"dependencies":{"a":"1"}}`), 0o600); err != nil {
		t.Fatal(err)
	}
	files := []selection.File{{Path: "package.json", Type: "json"}, {Path: "unused.ts", Type: "typescript"}}
	result, err := Run(root, config.Resolved{}, files, []string{"dependencies"})
	if err != nil || result.Summary.Checked != 1 || result.Summary.Unchecked != 1 || len(result.Diagnostics) != 0 {
		t.Fatalf("%v %v", result, err)
	}
	if value(t, result.Metrics, "uniqueDeclaredDependencies") != 1 {
		t.Fatal(result)
	}
	if Validate([]string{"llm"}) == nil || Validate([]string{"json", "json"}) == nil {
		t.Fatal("expected unavailable or duplicate check rejection")
	}
}

package selection

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"maintainablecheck/internal/config"
)

func project(t *testing.T, files map[string]string) string {
	t.Helper()
	root := t.TempDir()
	for name, source := range files {
		absolute := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(absolute), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(absolute, []byte(source), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	return root
}

func selectProject(t *testing.T, root string, requests ...string) Result {
	t.Helper()
	settings, err := config.Load(root, "")
	if err != nil {
		t.Fatal(err)
	}
	result, err := Select(root, requests, settings)
	if err != nil {
		t.Fatal(err)
	}
	return result
}

func assertFiles(t *testing.T, result Result, expected ...string) {
	t.Helper()
	actual := make([]string, 0, len(result.Files))
	for _, file := range result.Files {
		actual = append(actual, file.Path)
	}
	if !reflect.DeepEqual(actual, expected) {
		t.Fatalf("selected %v, expected %v", actual, expected)
	}
}

func TestGitIgnoreAnchorsNegationAndDirectories(t *testing.T) {
	root := project(t, map[string]string{
		".gitignore":          "ignored/\n*.secret.ts\n!visible.secret.ts\n/only-root.ts\n",
		"ignored/bad.ts":      "invalid",
		"hidden.secret.ts":    "invalid",
		"visible.secret.ts":   "valid",
		"only-root.ts":        "invalid",
		"nested/only-root.ts": "valid",
	})
	assertFiles(t, selectProject(t, root), "nested/only-root.ts", "visible.secret.ts")
}

func TestNestedGitIgnoreIsScopedAndInherited(t *testing.T) {
	root := project(t, map[string]string{
		".gitignore":             "*.secret.ts\n",
		"src/.gitignore":         "*.ts\n!keep.ts\n!keep.secret.ts\n",
		"src/hidden.ts":          "invalid",
		"src/keep.ts":            "valid",
		"src/keep.secret.ts":     "valid",
		"other/hidden.ts":        "valid",
		"other/hidden.secret.ts": "invalid",
	})
	assertFiles(t, selectProject(t, root), "other/hidden.ts", "src/keep.secret.ts", "src/keep.ts")
}

func TestDockerNegationReincludesChildAndNestedDockerFileIsNotContextRoot(t *testing.T) {
	root := project(t, map[string]string{
		".dockerignore":        "excluded/\n!excluded/keep.json\n**/temp*.ts\n",
		"excluded/keep.json":   "{}",
		"excluded/bad.json":    "invalid",
		"nested/.dockerignore": "ordinary.ts\n",
		"nested/ordinary.ts":   "valid",
		"nested/temp.ts":       "invalid",
	})
	assertFiles(t, selectProject(t, root), "excluded/keep.json", "nested/ordinary.ts")
	assertFiles(t, selectProject(t, root, "excluded/bad.json", "nested/ordinary.ts"), "nested/ordinary.ts")
}

func TestGitExcludedParentRemainsExcludedAcrossOtherPolicies(t *testing.T) {
	root := project(t, map[string]string{
		".gitignore":         "blocked/\n",
		".dockerignore":      "!blocked/keep.ts\n",
		"blocked/.gitignore": "!keep.ts\n",
		"blocked/keep.ts":    "invalid",
		"visible.ts":         "valid",
	})
	assertFiles(t, selectProject(t, root, "blocked/keep.ts", "visible.ts"), "visible.ts")
}

func TestCheckIgnoreSuffixDispatchAndExplicitScope(t *testing.T) {
	root := project(t, map[string]string{
		"checkignore.json":       `{"ignore":["skipped/**"],"extensions":{".generated.ts":false,".source":"typescript"}}`,
		"skipped/invalid.ts":     "invalid",
		"node_modules/vendor.ts": "invalid",
		"custom.source":          "valid",
		"upper.TS":               "valid",
		"view.generated.ts":      "invalid",
	})
	assertFiles(t, selectProject(t, root), "checkignore.json", "custom.source", "upper.TS")
	assertFiles(t, selectProject(t, root, "upper.TS"), "upper.TS")
	assertFiles(t, selectProject(t, root, ".", "upper.TS"), "checkignore.json", "custom.source", "upper.TS")
	settings, _ := config.Load(root, "")
	if _, err := Select(root, []string{"../outside.ts"}, settings); err == nil {
		t.Fatal("expected an out-of-root error")
	}
}

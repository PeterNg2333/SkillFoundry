package checks

import (
	"fmt"
	"path/filepath"

	"golang.org/x/mod/modfile"

	"maintainablecheck/internal/report"
)

type nodeManifest struct {
	Dependencies         map[string]string `json:"dependencies"`
	DevDependencies      map[string]string `json:"devDependencies"`
	PeerDependencies     map[string]string `json:"peerDependencies"`
	OptionalDependencies map[string]string `json:"optionalDependencies"`
}

func checkDependencies(file string, source []byte, limit *int) ([]report.Finding, []report.Metric) {
	if filepath.Base(file) == "go.mod" {
		return checkGoModules(file, source, limit)
	}
	var manifest nodeManifest
	if err := decodeJSON(source, &manifest); err != nil {
		return []report.Finding{finding(file, "error", "dependencies/manifest", err.Error(), 1, 1)}, nil
	}
	unique := map[string]bool{}
	metrics := []report.Metric{}
	groups := []struct {
		name   string
		values map[string]string
	}{
		{"runtimeDependencies", manifest.Dependencies},
		{"developmentDependencies", manifest.DevDependencies},
		{"peerDependencies", manifest.PeerDependencies},
		{"optionalDependencies", manifest.OptionalDependencies},
	}
	for _, group := range groups {
		metrics = append(metrics, metric(file, "dependencies", group.name, len(group.values), "packages"))
		for name := range group.values {
			unique[name] = true
		}
	}
	metrics = append(metrics, metric(file, "dependencies", "uniqueDeclaredDependencies", len(unique), "packages"))
	return dependencyLimit(file, len(unique), limit), metrics
}

func checkGoModules(file string, source []byte, limit *int) ([]report.Finding, []report.Metric) {
	manifest, err := modfile.Parse(file, source, nil)
	if err != nil {
		return []report.Finding{finding(file, "error", "dependencies/gomod", err.Error(), 1, 1)}, nil
	}
	direct, indirect := 0, 0
	for _, requirement := range manifest.Require {
		if requirement.Indirect {
			indirect++
		} else {
			direct++
		}
	}
	metrics := []report.Metric{
		metric(file, "dependencies", "directRequiredModules", direct, "modules"),
		metric(file, "dependencies", "indirectRequiredModules", indirect, "modules"),
	}
	return dependencyLimit(file, direct, limit), metrics
}

func dependencyLimit(file string, count int, limit *int) []report.Finding {
	if limit == nil || count <= *limit {
		return nil
	}
	message := fmt.Sprintf("Declared direct dependencies %d exceed configured limit %d.", count, *limit)
	return []report.Finding{finding(file, "warning", "dependencies/count", message, 1, 1)}
}

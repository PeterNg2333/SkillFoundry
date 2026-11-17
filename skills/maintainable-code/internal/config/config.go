package config

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	assets "maintainablecheck"
)

type Override struct {
	Files []string                   `json:"files"`
	Rules map[string]json.RawMessage `json:"rules"`
}

type Limits struct {
	DirectDependencies *int `json:"directDependencies,omitempty"`
	ImportsPerFile     *int `json:"importsPerFile,omitempty"`
	RequireComments    bool `json:"requireComments,omitempty"`
}

type Settings struct {
	Ignore     []string                   `json:"ignore"`
	Extensions map[string]json.RawMessage `json:"extensions"`
	Rules      map[string]json.RawMessage `json:"rules"`
	Overrides  []Override                 `json:"overrides"`
	Limits     Limits                     `json:"limits"`
}

type Resolved struct {
	Settings
	Defaults string  `json:"defaults"`
	Project  *string `json:"project"`
}

func decode(source []byte, name string) (Settings, error) {
	var settings Settings
	source = bytes.TrimSpace(bytes.TrimPrefix(source, []byte{0xef, 0xbb, 0xbf}))
	if len(source) == 0 || source[0] != '{' {
		return settings, fmt.Errorf("expected a configuration object in %s", name)
	}
	decoder := json.NewDecoder(bytes.NewReader(source))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&settings); err != nil {
		return settings, fmt.Errorf("configuration %s: %w", name, err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return settings, fmt.Errorf("configuration %s contains trailing JSON", name)
	}
	for _, pattern := range settings.Ignore {
		if pattern == "" || strings.HasPrefix(pattern, "!") {
			return settings, fmt.Errorf("configuration %s requires positive ignore globs", name)
		}
	}
	for extension, value := range settings.Extensions {
		if !strings.HasPrefix(extension, ".") || strings.ContainsAny(extension, `/\`) {
			return settings, fmt.Errorf("Unsupported extension mapping %s in %s", extension, name)
		}
		if _, err := ExtensionType(value); err != nil {
			return settings, fmt.Errorf("Unsupported extension mapping %s in %s: %w", extension, name, err)
		}
	}
	for _, override := range settings.Overrides {
		if len(override.Files) == 0 || override.Rules == nil {
			return settings, fmt.Errorf("configuration %s requires override files and rules", name)
		}
	}
	for _, limit := range []*int{settings.Limits.DirectDependencies, settings.Limits.ImportsPerFile} {
		if limit != nil && *limit < 0 {
			return settings, fmt.Errorf("configuration %s contains a negative limit", name)
		}
	}
	return settings, nil
}

func ExtensionType(value json.RawMessage) (string, error) {
	if string(bytes.TrimSpace(value)) == "false" {
		return "", nil
	}
	var kind string
	if err := json.Unmarshal(value, &kind); err != nil {
		return "", err
	}
	switch kind {
	case "javascript", "typescript", "json", "go", "gomod":
		return kind, nil
	default:
		return "", fmt.Errorf("unknown checker type %q", kind)
	}
}

func Load(root, requested string) (Resolved, error) {
	defaults, err := decode(assets.Defaults, "embedded checkignore.json")
	if err != nil {
		return Resolved{}, err
	}
	resolved := Resolved{Settings: defaults, Defaults: "embedded:checkignore.json"}
	name := requested
	if name == "" {
		name = "checkignore.json"
	}
	if !filepath.IsAbs(name) {
		name = filepath.Join(root, name)
	}
	source, err := os.ReadFile(name)
	if os.IsNotExist(err) && requested == "" {
		return resolved, nil
	}
	if err != nil {
		return resolved, fmt.Errorf("read configuration %s: %w", name, err)
	}
	project, err := decode(source, name)
	if err != nil {
		return resolved, err
	}
	resolved.Project = &name
	resolved.Ignore = append(resolved.Ignore, project.Ignore...)
	for extension, value := range project.Extensions {
		resolved.Extensions[strings.ToLower(extension)] = value
	}
	for rule, value := range project.Rules {
		resolved.Rules[rule] = value
	}
	resolved.Overrides = append(resolved.Overrides, project.Overrides...)
	resolved.Limits = project.Limits
	return resolved, nil
}

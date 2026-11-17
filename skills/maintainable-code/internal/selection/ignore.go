package selection

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/bmatcuk/doublestar/v4"
	"github.com/go-git/go-git/v5/plumbing/format/gitignore"
	"github.com/moby/patternmatcher"
	"github.com/moby/patternmatcher/ignorefile"
)

func dockerMatcher(root string) (*patternmatcher.PatternMatcher, error) {
	name := filepath.Join(root, ".dockerignore")
	source, err := os.ReadFile(name)
	if os.IsNotExist(err) {
		return patternmatcher.New(nil)
	}
	if err != nil {
		return nil, fmt.Errorf("read %s: %w", name, err)
	}
	patterns, err := ignorefile.ReadAll(bytes.NewReader(source))
	if err != nil {
		return nil, fmt.Errorf("parse %s: %w", name, err)
	}
	return patternmatcher.New(patterns)
}

func addGitPatterns(root, directory string, inherited []gitignore.Pattern) ([]gitignore.Pattern, error) {
	name := filepath.Join(root, filepath.FromSlash(directory), ".gitignore")
	source, err := os.ReadFile(name)
	if os.IsNotExist(err) {
		return inherited, nil
	}
	if err != nil {
		return nil, fmt.Errorf("read %s: %w", name, err)
	}
	patterns := append([]gitignore.Pattern(nil), inherited...)
	var domain []string
	if directory != "" {
		domain = strings.Split(directory, "/")
	}
	text := strings.TrimPrefix(string(source), "\ufeff")
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSuffix(line, "\r")
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		patterns = append(patterns, gitignore.ParsePattern(line, domain))
	}
	return patterns, nil
}

func matchesConfigured(relative string, patterns []string) bool {
	for _, pattern := range patterns {
		if doublestar.MatchUnvalidated(pattern, relative) || doublestar.MatchUnvalidated(pattern, relative+"/") {
			return true
		}
	}
	return false
}

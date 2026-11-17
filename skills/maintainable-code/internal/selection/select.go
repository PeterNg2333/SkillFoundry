package selection

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bmatcuk/doublestar/v4"
	"github.com/go-git/go-git/v5/plumbing/format/gitignore"
	"github.com/moby/patternmatcher"

	"maintainablecheck/internal/config"
)

type File struct {
	Path string `json:"path"`
	Type string `json:"type"`
}

type Skipped struct {
	Path   string `json:"path"`
	Reason string `json:"reason"`
}

type Result struct {
	Files   []File    `json:"files"`
	Skipped []Skipped `json:"skipped"`
}

type selector struct {
	root       string
	settings   config.Resolved
	docker     *patternmatcher.PatternMatcher
	requests   []string
	extensions []string
	result     Result
}

func Select(root string, requested []string, settings config.Resolved) (Result, error) {
	s := selector{root: root, settings: settings, result: Result{Files: []File{}, Skipped: []Skipped{}}}
	for _, pattern := range settings.Ignore {
		if !doublestar.ValidatePattern(pattern) {
			return s.result, fmt.Errorf("invalid checkignore glob %q", pattern)
		}
	}
	allRequested := false
	for _, request := range requested {
		absolute := request
		if !filepath.IsAbs(absolute) {
			absolute = filepath.Join(root, request)
		}
		relative, err := filepath.Rel(root, absolute)
		if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) || filepath.IsAbs(relative) {
			return s.result, fmt.Errorf("requested path is outside the check root %s", request)
		}
		if _, err := os.Lstat(absolute); err != nil {
			return s.result, fmt.Errorf("requested path %s: %w", request, err)
		}
		if relative != "." {
			s.requests = append(s.requests, filepath.ToSlash(relative))
		} else {
			allRequested = true
		}
	}
	if allRequested {
		s.requests = nil
	}
	for extension := range settings.Extensions {
		s.extensions = append(s.extensions, extension)
	}
	sort.Slice(s.extensions, func(i, j int) bool { return len(s.extensions[i]) > len(s.extensions[j]) })
	var err error
	s.docker, err = dockerMatcher(root)
	if err != nil {
		return s.result, err
	}
	err = s.visit("", nil)
	return s.result, err
}

func (s *selector) inScope(relative string, directory bool) bool {
	if relative == "" || len(s.requests) == 0 {
		return true
	}
	for _, request := range s.requests {
		if relative == request || strings.HasPrefix(relative, request+"/") || (directory && strings.HasPrefix(request, relative+"/")) {
			return true
		}
	}
	return false
}

func (s *selector) skip(relative, reason string) {
	s.result.Skipped = append(s.result.Skipped, Skipped{Path: relative, Reason: reason})
}

func (s *selector) visit(relative string, patterns []gitignore.Pattern) error {
	absolute := filepath.Join(s.root, filepath.FromSlash(relative))
	entry, err := os.Lstat(absolute)
	if err != nil {
		return fmt.Errorf("inspect %s: %w", relative, err)
	}
	if !s.inScope(relative, entry.IsDir()) {
		return nil
	}
	if relative != "" && matchesConfigured(relative, s.settings.Ignore) {
		s.skip(relative, "ignore")
		return nil
	}
	if entry.Mode()&os.ModeSymlink != 0 {
		s.skip(relative, "symlink")
		return nil
	}
	if relative != "" && gitignore.NewMatcher(patterns).Match(strings.Split(relative, "/"), entry.IsDir()) {
		s.skip(relative, "gitignore")
		return nil
	}
	excluded, err := s.docker.MatchesOrParentMatches(relative)
	if err != nil {
		return fmt.Errorf("dockerignore %s: %w", relative, err)
	}
	if relative != "" && excluded {
		s.skip(relative, "dockerignore")
		// Docker negations can restore descendants even while their parent is excluded.
		if !entry.IsDir() || !s.docker.Exclusions() {
			return nil
		}
	}
	if entry.IsDir() {
		return s.visitDirectory(relative, patterns)
	}
	if !entry.Mode().IsRegular() {
		return nil
	}
	for _, extension := range s.extensions {
		if !strings.HasSuffix(strings.ToLower(relative), extension) {
			continue
		}
		kind, err := config.ExtensionType(s.settings.Extensions[extension])
		if err != nil {
			return err
		}
		if kind == "" {
			s.skip(relative, "disabled-extension")
			return nil
		}
		s.result.Files = append(s.result.Files, File{Path: relative, Type: kind})
		return nil
	}
	s.skip(relative, "unsupported-extension")
	return nil
}

func (s *selector) visitDirectory(relative string, inherited []gitignore.Pattern) error {
	patterns, err := addGitPatterns(s.root, relative, inherited)
	if err != nil {
		return err
	}
	entries, err := os.ReadDir(filepath.Join(s.root, filepath.FromSlash(relative)))
	if err != nil {
		return fmt.Errorf("read directory %s: %w", relative, err)
	}
	for _, entry := range entries {
		name := entry.Name()
		if relative != "" {
			name = relative + "/" + name
		}
		if err := s.visit(name, patterns); err != nil {
			return err
		}
	}
	return nil
}

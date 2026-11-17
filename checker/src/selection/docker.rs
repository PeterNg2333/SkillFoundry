use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use globset::GlobMatcher;

use crate::config::glob;

struct Pattern {
    matcher: GlobMatcher,
    negated: bool,
}

/// Docker patterns are rooted in the context and can restore an excluded child.
/// Git's directory pruning and basename matching are deliberately kept separate.
pub struct DockerIgnore {
    patterns: Vec<Pattern>,
    pub has_negations: bool,
}

impl DockerIgnore {
    pub fn load(root: &Path) -> Result<Self> {
        let file = root.join(".dockerignore");
        let source = match fs::read_to_string(&file) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error).with_context(|| format!("read {}", file.display())),
        };
        let mut patterns = Vec::new();
        for line in source.trim_start_matches('\u{feff}').lines() {
            if line.starts_with('#') {
                continue;
            }
            let line = line.trim();
            if line.is_empty() || line == "." {
                continue;
            }
            let negated = line.starts_with('!');
            let line = if negated { line[1..].trim() } else { line };
            ensure!(!line.is_empty(), "invalid exclusion in .dockerignore");
            let mut parts = Vec::new();
            for part in line.trim_matches('/').split('/') {
                match part {
                    "." | "" => {}
                    ".." => {
                        parts.pop();
                    }
                    _ => parts.push(part),
                }
            }
            if !parts.is_empty() {
                patterns.push(Pattern {
                    matcher: glob(&parts.join("/")).context("invalid .dockerignore pattern")?,
                    negated,
                });
            }
        }
        let has_negations = patterns.iter().any(|pattern| pattern.negated);
        Ok(Self {
            patterns,
            has_negations,
        })
    }

    pub fn excludes(&self, relative: &str) -> bool {
        let mut excluded = false;
        for pattern in &self.patterns {
            let mut candidate = relative;
            loop {
                if pattern.matcher.is_match(candidate) {
                    excluded = !pattern.negated;
                    break;
                }
                let Some((parent, _)) = candidate.rsplit_once('/') else {
                    break;
                };
                candidate = parent;
            }
        }
        excluded
    }
}

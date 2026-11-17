mod docker;

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use globset::GlobMatcher;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use serde::Serialize;

use crate::config::{Resolved, glob};
use docker::DockerIgnore;

#[derive(Serialize)]
pub struct File {
    pub path: String,
    #[serde(rename = "type")]
    pub kind: String,
}
#[derive(Serialize)]
pub struct Skipped {
    pub path: String,
    pub reason: &'static str,
}
#[derive(Default)]
pub struct Selection {
    pub files: Vec<File>,
    pub skipped: Vec<Skipped>,
}

struct Selector<'a> {
    root: &'a Path,
    settings: &'a Resolved,
    ignores: Vec<GlobMatcher>,
    extensions: Vec<String>,
    requests: Vec<String>,
    docker: DockerIgnore,
    result: Selection,
}

pub(crate) fn normalized(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other),
        }
    }
    result
}

pub fn select(root: &Path, requests: &[PathBuf], settings: &Resolved) -> Result<Selection> {
    let mut scope = Vec::new();
    for request in requests {
        let absolute = normalized(&root.join(request));
        let relative = absolute
            .strip_prefix(root)
            .context("requested path is outside the check root")?;
        fs::symlink_metadata(&absolute)
            .with_context(|| format!("requested path {}", request.display()))?;
        scope.push(relative.to_string_lossy().replace('\\', "/"));
    }
    if scope.iter().any(String::is_empty) {
        scope.clear();
    }
    let mut extensions: Vec<_> = settings.settings.extensions.keys().cloned().collect();
    extensions.sort_by_key(|extension| (std::cmp::Reverse(extension.len()), extension.clone()));
    let mut selector = Selector {
        root,
        settings,
        requests: scope,
        extensions,
        ignores: settings
            .settings
            .ignore
            .iter()
            .map(|pattern| glob(pattern))
            .collect::<Result<_>>()?,
        docker: DockerIgnore::load(root)?,
        result: Selection::default(),
    };
    selector.visit("", &[])?;
    Ok(selector.result)
}

impl Selector<'_> {
    fn skip(&mut self, path: &str, reason: &'static str) {
        self.result.skipped.push(Skipped {
            path: path.to_owned(),
            reason,
        });
    }

    fn visit(&mut self, relative: &str, inherited: &[Gitignore]) -> Result<()> {
        let absolute = self.root.join(relative);
        let metadata = fs::symlink_metadata(&absolute)
            .with_context(|| format!("inspect {}", absolute.display()))?;
        if !self.requests.is_empty()
            && !relative.is_empty()
            && !self.requests.iter().any(|request| {
                relative == request
                    || relative.starts_with(&format!("{request}/"))
                    || (metadata.is_dir() && request.starts_with(&format!("{relative}/")))
            })
        {
            return Ok(());
        }
        if self
            .ignores
            .iter()
            .any(|matcher| matcher.is_match(relative) || matcher.is_match(format!("{relative}/")))
        {
            self.skip(relative, "ignore");
            return Ok(());
        }
        if metadata.file_type().is_symlink() {
            self.skip(relative, "symlink");
            return Ok(());
        }
        let git_match = inherited
            .iter()
            .rev()
            .map(|matcher| matcher.matched(&absolute, metadata.is_dir()))
            .find(|matched| !matched.is_none());
        if git_match.is_some_and(|matched| matched.is_ignore()) {
            self.skip(relative, "gitignore");
            return Ok(());
        }
        if self.docker.excludes(relative) {
            self.skip(relative, "dockerignore");
            if !metadata.is_dir() || !self.docker.has_negations {
                return Ok(());
            }
        }
        if metadata.is_dir() {
            return self.visit_directory(relative, inherited);
        }
        if !metadata.is_file() {
            return Ok(());
        }
        for extension in &self.extensions {
            if !relative.to_lowercase().ends_with(extension) {
                continue;
            }
            let kind = &self.settings.settings.extensions[extension];
            if kind == false {
                self.skip(relative, "disabled-extension");
                return Ok(());
            }
            self.result.files.push(File {
                path: relative.to_owned(),
                kind: kind.as_str().context("invalid extension kind")?.to_owned(),
            });
            return Ok(());
        }
        self.skip(relative, "unsupported-extension");
        Ok(())
    }

    fn visit_directory(&mut self, relative: &str, inherited: &[Gitignore]) -> Result<()> {
        let directory = self.root.join(relative);
        let file = directory.join(".gitignore");
        let mut patterns = inherited.to_vec();
        match fs::metadata(&file) {
            Ok(metadata) => {
                ensure!(metadata.is_file(), ".gitignore must be a file");
                let mut builder = GitignoreBuilder::new(&directory);
                if let Some(error) = builder.add(&file) {
                    return Err(error).context("read .gitignore");
                }
                patterns.push(builder.build().context("parse .gitignore")?);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("inspect .gitignore"),
        }
        let mut entries = fs::read_dir(&directory)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("non-UTF-8 path in {}", directory.display()))?;
            let child = if relative.is_empty() {
                name
            } else {
                format!("{relative}/{name}")
            };
            self.visit(&child, &patterns)?;
        }
        Ok(())
    }
}

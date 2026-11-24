use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use globset::GlobMatcher;
use serde::{Deserialize, Serialize};

use super::glob;

#[derive(Clone, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct FileStructurePattern {
    text: String,
    matcher: GlobMatcher,
}

impl TryFrom<String> for FileStructurePattern {
    type Error = anyhow::Error;

    fn try_from(text: String) -> Result<Self> {
        let pattern = text.strip_prefix("./").unwrap_or(&text);
        ensure!(
            !pattern.trim().is_empty()
                && !pattern.starts_with(['!', '/'])
                && !pattern.split('/').any(|part| part == ".."),
            "invalid fileStructure glob {text:?}"
        );
        let matcher =
            glob(pattern).with_context(|| format!("invalid fileStructure glob {text:?}"))?;
        Ok(Self { text, matcher })
    }
}

impl From<FileStructurePattern> for String {
    fn from(pattern: FileStructurePattern) -> Self {
        pattern.text
    }
}

impl FileStructurePattern {
    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn matches(&self, name: &str, path: &str) -> bool {
        self.matcher
            .is_match(if self.text.contains('/') { path } else { name })
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FileStructureIgnore {
    Flag(bool),
    Patterns(Vec<FileStructurePattern>),
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FileStructure {
    #[serde(rename = "$root")]
    root: Option<String>,
    #[serde(rename = "$accepts")]
    pub accepts: Vec<FileStructurePattern>,
    #[serde(rename = "$ignore")]
    pub ignore: Option<FileStructureIgnore>,
    #[serde(rename = "$description")]
    pub description: Option<String>,
    #[serde(rename = "/")]
    pub children: Option<BTreeMap<String, FileStructure>>,
}

fn literal_name(name: &str) -> bool {
    !matches!(name, "" | "." | "..") && !name.contains(['/', '\\', ':', '*', '?', '\0'])
}

impl FileStructure {
    pub fn root(&self) -> &str {
        self.root.as_deref().unwrap_or(".")
    }

    pub fn ignored(&self) -> bool {
        matches!(self.ignore, Some(FileStructureIgnore::Flag(true)))
    }

    pub fn validate(&self) -> Result<()> {
        let root = self.root();
        ensure!(
            root == "." || root.split('/').all(literal_name),
            "$root must be a relative directory path"
        );
        ensure!(
            self.children.is_some(),
            "the root directory requires a \"/\" object"
        );
        self.validate_children()
    }

    fn validate_children(&self) -> Result<()> {
        let Some(children) = &self.children else {
            ensure!(
                self.accepts.is_empty()
                    && !matches!(self.ignore, Some(FileStructureIgnore::Patterns(_))),
                "file entries only support a boolean $ignore and $description"
            );
            return Ok(());
        };
        for (name, entry) in children {
            ensure!(
                literal_name(name),
                "{name:?} must be a literal file or directory name in \"/\""
            );
            ensure!(
                entry.root.is_none(),
                "$root is only allowed at the policy root"
            );
            entry
                .validate_children()
                .with_context(|| format!("entry {name:?}"))?;
        }
        Ok(())
    }
}

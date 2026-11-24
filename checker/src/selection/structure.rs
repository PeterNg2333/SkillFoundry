use termtree::Tree;

use crate::config::{FileStructure, FileStructureIgnore, FileStructurePattern};

use super::{File, Selection, Skipped};

pub struct Violation {
    pub path: String,
    pub message: String,
}

pub struct StructureSelection {
    pub selection: Selection,
    pub violations: Vec<Violation>,
    pub inspected: bool,
}

pub(super) struct Scan<'a> {
    policy: &'a FileStructure,
    root: &'a str,
    anchor_seen: bool,
    inspected: bool,
    violations: Vec<Violation>,
}

enum Decision {
    Accept,
    Ignore,
    Reject(String),
}

fn in_tree<'a>(path: &'a str, root: &str) -> Option<&'a str> {
    if root.is_empty() {
        return Some(path);
    }
    let relative = path.strip_prefix(root)?;
    if relative.is_empty() {
        Some(relative)
    } else {
        relative.strip_prefix('/')
    }
}

impl<'a> Scan<'a> {
    pub(super) fn new(policy: &'a FileStructure) -> Self {
        let root = policy.root();
        Self {
            policy,
            root: if root == "." { "" } else { root },
            anchor_seen: false,
            inspected: false,
            violations: Vec::new(),
        }
    }

    pub(super) fn includes(&self, path: &str) -> bool {
        in_tree(path, self.root).is_some() || in_tree(self.root, path).is_some()
    }

    pub(super) fn skip(&mut self, path: &str) {
        self.anchor_seen |= in_tree(self.root, path).is_some();
    }

    pub(super) fn inspect(
        &mut self,
        path: &str,
        directory: bool,
        selected: &mut Selection,
    ) -> bool {
        let Some(relative) = in_tree(path, self.root) else {
            return true;
        };
        self.anchor_seen = true;
        let decision = decide(self.policy, relative, directory);
        if matches!(decision, Decision::Ignore) {
            selected.skipped.push(Skipped {
                path: path.to_owned(),
                reason: "file-structure-ignore",
            });
            return false;
        }
        self.inspected = true;
        if !directory {
            selected.files.push(File {
                path: path.to_owned(),
                kind: "structure".to_owned(),
            });
        }
        if let Decision::Reject(message) = decision {
            self.violations.push(Violation {
                path: path.to_owned(),
                message,
            });
            return false;
        }
        true
    }

    pub(super) fn finish(
        mut self,
        selection: Selection,
        requests: &[String],
    ) -> StructureSelection {
        if !self.anchor_seen
            && (requests.is_empty() || requests.iter().any(|path| self.includes(path)))
        {
            self.violations.push(Violation {
                path: self.root.to_owned(),
                message: "the directory configured as fileStructure.$root was not found".to_owned(),
            });
        }
        StructureSelection {
            selection,
            inspected: self.inspected || !self.violations.is_empty(),
            violations: self.violations,
        }
    }
}

fn exact(entry: &FileStructure, is_directory: bool) -> Decision {
    match (entry.children.is_some(), is_directory) {
        (true, false) => Decision::Reject("expected a directory declared with \"/\"".to_owned()),
        (false, true) => Decision::Reject("expected a file declared without \"/\"".to_owned()),
        _ if entry.ignored() => Decision::Ignore,
        _ => Decision::Accept,
    }
}

fn decide(root: &FileStructure, relative: &str, is_directory: bool) -> Decision {
    if relative.is_empty() {
        return exact(root, is_directory);
    }
    let mut scope = root;
    let mut remaining = relative;
    let mut ancestors = Vec::new();
    while let Some((name, rest)) = remaining.split_once('/') {
        let Some(child) = scope
            .children
            .as_ref()
            .and_then(|entries| entries.get(name))
        else {
            return Decision::Reject(
                "parent directory is not declared in fileStructure".to_owned(),
            );
        };
        ancestors.push((scope, remaining));
        scope = child;
        remaining = rest;
    }
    if let Some(entry) = scope
        .children
        .as_ref()
        .and_then(|entries| entries.get(remaining))
    {
        return exact(entry, is_directory);
    }
    if is_directory {
        return Decision::Reject(
            "directory is not declared in fileStructure. Add it under \"/\".".to_owned(),
        );
    }

    let name = remaining;
    ancestors.push((scope, remaining));
    for &(scope, below_scope) in ancestors.iter().rev() {
        if let Some(FileStructureIgnore::Patterns(patterns)) = &scope.ignore
            && patterns
                .iter()
                .any(|pattern| pattern.matches(name, below_scope))
        {
            return Decision::Ignore;
        }
        if scope
            .accepts
            .iter()
            .any(|pattern| pattern.matches(name, below_scope))
        {
            return Decision::Accept;
        }
    }
    let patterns = ancestors
        .iter()
        .rev()
        .flat_map(|(scope, path)| {
            scope
                .accepts
                .iter()
                .map(FileStructurePattern::as_str)
                .filter(|pattern| {
                    !path.contains('/')
                        || pattern
                            .strip_prefix("./")
                            .is_none_or(|pattern| pattern.contains('/'))
                })
        })
        .collect::<Vec<_>>()
        .join(", ");
    let accepts = if patterns.is_empty() {
        "no $accepts patterns apply".to_owned()
    } else {
        format!("$accepts patterns: {patterns}")
    };
    Decision::Reject(format!(
        "file does not match fileStructure ({accepts}). Declare it under \"/\" or update $accepts."
    ))
}

pub fn tree(policy: &FileStructure) -> Tree<String> {
    fn branch(name: &str, entry: &FileStructure) -> Tree<String> {
        let slash = if entry.children.is_some() { "/" } else { "" };
        let mut label = format!("{name}{slash}");
        if entry.ignored() {
            label.push_str("  [ignored]");
        }
        let mut patterns_label = |patterns: &[FileStructurePattern], verb: &str| {
            if !patterns.is_empty() {
                let patterns = patterns
                    .iter()
                    .map(FileStructurePattern::as_str)
                    .collect::<Vec<_>>();
                label.push_str(&format!("  [{verb} {}]", patterns.join(", ")));
            }
        };
        patterns_label(&entry.accepts, "accepts");
        if let Some(FileStructureIgnore::Patterns(patterns)) = &entry.ignore {
            patterns_label(patterns, "ignores");
        }
        if let Some(description) = &entry.description {
            label.push_str("  ");
            label.push_str(description);
        }
        let children = entry
            .children
            .iter()
            .flatten()
            .map(|(name, child)| branch(name, child));
        Tree::new(label).with_leaves(children).with_multiline(true)
    }
    branch(policy.root(), policy)
}

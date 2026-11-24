mod ast;
mod directives;
mod javascript;
mod loop_similarity;
mod manifests;
mod metrics;
mod semantics;
mod tools;
mod ui;

use crate::{
    config::{RULES, Resolved, TOOL_CHECKS, validate_rule_filter},
    report::{CheckStatus, Report},
    selection,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

pub struct SourceFile<'a> {
    pub project: &'a Path,
    pub path: &'a str,
    pub text: &'a str,
    pub kind: &'a str,
}

pub const DEFAULT_CHECKS: &[&str] = &[
    "json",
    "toml",
    "dependencies",
    "go",
    "javascript",
    "typescript",
];
pub const FILE_STRUCTURE: &str = "file-structure";
const STRUCTURE_PATH: &str = "file-structure/path";
const CATALOG: &[&str] = &[
    "json",
    "toml",
    "dependencies",
    "go",
    "javascript",
    "typescript",
    "comments",
    "directives",
    FILE_STRUCTURE,
];

pub fn capabilities() -> Value {
    let checks: Vec<_> = CATALOG.iter().chain(TOOL_CHECKS).copied().collect();
    json!({"engine":"rust", "version":env!("CARGO_PKG_VERSION"), "checks":checks, "rules":RULES,
        "externalChecks":{"dependency-boundaries":"dependency-cruiser","duplication":"jscpd","python":"ruff","go-format":"gofmt"},
        "libraries":["ast-grep", "Oxc", "ignore", "globset", "serde_json", "termtree", "toml"],
        "unsupported":[{"id":"typecheck","reason":"Syntax and local binding analysis do not replace the project's type checker."}],
        "modelReview":{"implemented":false,"default":"off"}})
}

pub fn validate(names: &[String]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for name in names {
        ensure!(
            CATALOG.contains(&name.as_str()) || TOOL_CHECKS.contains(&name.as_str()),
            "unknown check {name}"
        );
        ensure!(seen.insert(name), "duplicate check {name}");
    }
    Ok(())
}

pub fn run(
    root: &Path,
    paths: &[PathBuf],
    settings: &Resolved,
    names: &[String],
    rules: &[String],
    report: &mut Report,
) -> Result<()> {
    validate(names)?;
    validate_rule_filter(rules)?;
    let selected = |name: &str| names.iter().any(|selected| selected == name);
    ensure!(
        rules.is_empty() || selected("javascript") || selected("typescript"),
        "--rules requires a javascript or typescript check"
    );
    let mut counts = BTreeMap::new();
    let mut checked = BTreeSet::new();
    let mut loop_files = Vec::new();
    for (file, kind) in report
        .files
        .iter()
        .map(|file| (file.path.clone(), file.kind.clone()))
        .collect::<Vec<_>>()
    {
        let manifest = matches!(
            Path::new(&file).file_name().and_then(|name| name.to_str()),
            Some("package.json" | "go.mod" | "Cargo.toml")
        );
        let main = selected(&kind) && kind != "gomod" && !TOOL_CHECKS.contains(&kind.as_str());
        let dependencies = manifest && selected("dependencies");
        let comments =
            matches!(kind.as_str(), "go" | "javascript" | "typescript") && selected("comments");
        let directives =
            matches!(kind.as_str(), "javascript" | "typescript") && selected("directives");
        if !main && !dependencies && !comments && !directives {
            continue;
        }
        let source =
            fs::read_to_string(root.join(&file)).with_context(|| format!("read {file}"))?;
        let source = source.trim_start_matches('\u{feff}');
        if dependencies {
            manifests::dependencies(&file, source, &settings.settings.limits, report);
            *counts.entry("dependencies".to_owned()).or_insert(0) += 1;
        }
        if main || comments || directives {
            match kind.as_str() {
                "json" => manifests::json(&file, source, report),
                "toml" => manifests::toml(&file, source, report),
                "go" => ast::check_go(
                    &file,
                    source,
                    &settings.settings.limits,
                    main,
                    comments,
                    report,
                )?,
                "javascript" | "typescript" => {
                    if let Some(file) = javascript::check(
                        &SourceFile {
                            project: root,
                            path: &file,
                            text: source,
                            kind: &kind,
                        },
                        &settings.settings,
                        main,
                        comments,
                        directives,
                        rules,
                        report,
                    )? {
                        loop_files.push(file);
                    }
                }
                _ => {}
            }
            if main {
                *counts.entry(kind).or_insert(0) += 1;
            }
            if comments {
                *counts.entry("comments".to_owned()).or_insert(0) += 1;
            }
            if directives {
                *counts.entry("directives".to_owned()).or_insert(0) += 1;
            }
        }
        checked.insert(file);
    }
    loop_similarity::finish(root, loop_files, settings.settings.loop_similarity, report);
    for name in names {
        if name == FILE_STRUCTURE {
            continue;
        }
        if TOOL_CHECKS.contains(&name.as_str()) {
            tools::run(name, root, &settings.settings, report, &mut checked)?;
            continue;
        }
        let files = counts.get(name).copied().unwrap_or(0);
        report.checks.push(CheckStatus {
            id: name.clone(),
            status: if files == 0 {
                "not-applicable"
            } else {
                "checked"
            },
            files,
            reason: None,
        });
    }
    if selected("go") && !selected("go-format") && report.files.iter().any(|file| file.kind == "go")
    {
        report.checks.push(CheckStatus {
            id: "go-format".to_owned(),
            status: "unsupported",
            files: 0,
            reason: Some("Enable the go-format check to run gofmt.".to_owned()),
        });
    }
    if selected(FILE_STRUCTURE) {
        check_structure(root, paths, settings, report, &mut checked)?;
    }
    report.summary.checked = checked.len();
    report.summary.unchecked = report.files.len() - report.summary.checked;
    Ok(())
}

fn check_structure(
    root: &Path,
    paths: &[PathBuf],
    settings: &Resolved,
    report: &mut Report,
    checked: &mut BTreeSet<String>,
) -> Result<()> {
    let mut status = CheckStatus {
        id: FILE_STRUCTURE.to_owned(),
        status: "not-applicable",
        files: 0,
        reason: Some("No fileStructure policy is configured.".to_owned()),
    };
    if let Some(policy) = &settings.settings.file_structure {
        let result = selection::select_structure(root, paths, settings, policy)?;
        status.files = result.selection.files.len();
        if result.inspected {
            status.status = "checked";
            status.reason = None;
        } else {
            status.reason =
                Some("No fileStructure entries were inspected in this scope.".to_owned());
        }
        for violation in result.violations {
            report.finding(
                &violation.path,
                "",
                0,
                "error",
                STRUCTURE_PATH,
                violation.message,
            );
        }
        checked.extend(result.selection.files.iter().map(|file| file.path.clone()));
        report.files.extend(result.selection.files);
        report
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        report.files.dedup_by(|left, right| left.path == right.path);
        report.skipped.retain(|entry| {
            entry.reason != "unsupported-extension" || !checked.contains(&entry.path)
        });
        report.skipped.extend(result.selection.skipped);
        report
            .skipped
            .sort_by(|left, right| (&left.path, left.reason).cmp(&(&right.path, right.reason)));
        report
            .skipped
            .dedup_by(|left, right| left.path == right.path && left.reason == right.reason);
    }
    report.checks.push(status);
    Ok(())
}

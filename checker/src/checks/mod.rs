mod ast;
mod directives;
mod javascript;
mod manifests;
mod metrics;
mod semantics;

use crate::{
    config::{RULES, Resolved, validate_rule_filter},
    report::{CheckStatus, Report},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
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
const CATALOG: &[&str] = &[
    "json",
    "toml",
    "dependencies",
    "go",
    "javascript",
    "typescript",
    "comments",
    "directives",
];

pub fn capabilities() -> Value {
    json!({"engine":"rust", "version":env!("CARGO_PKG_VERSION"), "checks":CATALOG, "rules":RULES,
        "libraries":["ast-grep", "Oxc", "ignore", "globset", "serde_json", "toml"],
        "unsupported":[{"id":"go-format","reason":"The native Rust checker does not implement gofmt. Run the project's Go formatter separately."},
        {"id":"typecheck","reason":"Syntax and local binding analysis do not replace the project's type checker."}],
        "modelReview":{"implemented":false,"default":"off"}})
}

pub fn validate(names: &[String]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for name in names {
        ensure!(CATALOG.contains(&name.as_str()), "unknown check {name}");
        ensure!(seen.insert(name), "duplicate check {name}");
    }
    Ok(())
}

pub fn run(
    root: &Path,
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
        let main = selected(&kind) && kind != "gomod";
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
                "javascript" | "typescript" => javascript::check(
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
                )?,
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
        report.summary.checked += 1;
    }
    report.summary.unchecked = report.files.len() - report.summary.checked;
    for name in names {
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
    if selected("go") && report.files.iter().any(|file| file.kind == "go") {
        report.checks.push(CheckStatus {
            id: "go-format".to_owned(),
            status: "unsupported",
            files: 0,
            reason: Some("Run the project's gofmt check separately.".to_owned()),
        });
    }
    Ok(())
}

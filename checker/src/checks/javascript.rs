use std::{collections::BTreeSet, path::Path};

use anyhow::{Context, Result};
use ast_grep_config::{GlobalRules, from_yaml_string};
use ast_grep_language::SupportLang;

use super::{
    SourceFile,
    ast::{self, SyntaxNode},
    metrics, semantics,
};
use crate::{
    config::{Policy, Settings},
    report::Report,
};

fn emit(
    node: &SyntaxNode<'_>,
    file: &str,
    source: &str,
    policy: &Policy,
    rule: &str,
    message: &str,
    report: &mut Report,
) {
    if let Some(severity) = policy.severity(rule) {
        report.finding(file, source, node.range().start, severity, rule, message);
    }
}

fn check_exports(
    root: &SyntaxNode<'_>,
    file: &str,
    source: &str,
    policy: &Policy,
    report: &mut Report,
) {
    let mut seen_export = false;
    for node in root
        .named_children()
        .filter(|node| node.kind() != "comment")
    {
        if node.kind() != "export_statement" {
            if seen_export {
                emit(
                    &node,
                    file,
                    source,
                    policy,
                    "no-restricted-syntax",
                    "Keep public exports together at the end of the file.",
                    report,
                );
            }
            continue;
        }
        seen_export = true;
        if node.field("declaration").is_some()
            || node.field("value").is_some_and(|value| {
                matches!(
                    value.kind().as_ref(),
                    "arrow_function" | "function_expression" | "class"
                )
            })
        {
            emit(
                &node,
                file,
                source,
                policy,
                "no-restricted-syntax",
                "Declare values and types first; collect exports at the end.",
                report,
            );
        }
        let is_type_export = node.children().any(|child| child.kind() == "type");
        if !is_type_export
            && node.dfs().any(|child| {
                child.kind() == "export_specifier"
                    && child.children().any(|token| token.kind() == "type")
            })
        {
            emit(
                &node,
                file,
                source,
                policy,
                "no-restricted-syntax",
                "Collect type exports in a separate export type list.",
                report,
            );
        }
    }
}

fn check_imports(
    root: &SyntaxNode<'_>,
    project: &Path,
    file: &str,
    source: &str,
    policy: &Policy,
    report: &mut Report,
) -> usize {
    let mut imports = 0;
    for node in root.dfs() {
        let source_node = match node.kind().as_ref() {
            "import_statement" | "export_statement" => node.field("source"),
            "call_expression"
                if node
                    .field("function")
                    .is_some_and(|function| function.kind() == "import") =>
            {
                node.field("arguments")
                    .and_then(|arguments| arguments.named_children().next())
            }
            _ => None,
        };
        let Some(source_node) = source_node.filter(|node| node.kind() == "string") else {
            continue;
        };
        imports += 1;
        let specifier = source_node.text();
        let specifier = specifier[1..specifier.len() - 1].replace("\\\\", "/");
        if !specifier.starts_with('.') || matches!(specifier.as_str(), "." | "./") {
            continue;
        }
        let destination = Path::new(&specifier);
        let directory = project.join(file).parent().unwrap().to_path_buf();
        let destination = crate::selection::normalized(&directory.join(destination));
        let crosses_directory =
            destination.parent() != Some(directory.as_path()) || destination.is_dir();
        if crosses_directory {
            emit(
                &source_node,
                file,
                source,
                policy,
                "maintainable/cross-directory-imports",
                "Use a project alias for cross-directory imports and re-exports.",
                report,
            );
        }
    }
    imports
}

pub fn check(
    input: &SourceFile<'_>,
    settings: &Settings,
    inspect: bool,
    inspect_comments: bool,
    report: &mut Report,
) -> Result<()> {
    let SourceFile {
        project,
        path: file,
        text: source,
        kind,
    } = *input;
    let policy = Policy::for_file(settings, file)?;
    if !semantics::check(file, source, kind == "typescript", inspect, &policy, report) {
        return Ok(());
    }
    let (language, label) = match (kind, file.to_lowercase().ends_with(".tsx")) {
        ("typescript", true) => (SupportLang::Tsx, "Tsx"),
        ("typescript", false) => (SupportLang::TypeScript, "TypeScript"),
        _ => (SupportLang::JavaScript, "JavaScript"),
    };
    let tree = ast::parse(source, language)?;
    let root = tree.root();
    if inspect {
        let rules = from_yaml_string::<SupportLang>(
            &include_str!("../../rules/javascript.yml")
                .replace("language: TypeScript", &format!("language: {label}")),
            &GlobalRules::default(),
        )
        .context("compile embedded AST rules")?;
        let overloads: BTreeSet<(usize, String)> = root
            .dfs()
            .filter(|node| node.kind() == "function_signature")
            .filter_map(|node| {
                node.parent()
                    .zip(node.field("name"))
                    .map(|(parent, name)| (parent.node_id(), name.text().into_owned()))
            })
            .collect();
        for rule in rules {
            if policy.severity(&rule.id).is_none() {
                continue;
            }
            for matched in root.find_all(&rule.matcher) {
                if rule.id == "func-style"
                    && matched
                        .parent()
                        .zip(matched.field("name"))
                        .is_some_and(|(parent, name)| {
                            overloads.contains(&(parent.node_id(), name.text().into_owned()))
                        })
                {
                    continue;
                }
                emit(
                    &matched,
                    file,
                    source,
                    &policy,
                    &rule.id,
                    &rule.message,
                    report,
                );
            }
        }
        check_exports(&root, file, source, &policy, report);
        let imports = check_imports(&root, project, file, source, &policy, report);
        report.metric(file, kind, "imports", imports);
        if settings
            .limits
            .imports_per_file
            .is_some_and(|limit| imports > limit)
        {
            report.finding(
                file,
                source,
                0,
                "warning",
                "imports/count",
                format!("{imports} imports exceed the configured limit."),
            );
        }
        metrics::check(&root, input, &policy, report);
    }
    if inspect_comments {
        let (groups, _) = ast::comments(&root, source);
        ast::check_comments(file, source, groups, &settings.limits, report);
    }
    Ok(())
}

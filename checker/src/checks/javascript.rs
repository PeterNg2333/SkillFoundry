//! JavaScript and TypeScript adapter: grammar roles for the shared rules and the
//! JS/TS-only structural rules (embedded ast-grep rules and export organization).

use std::{collections::BTreeSet, ops::Range};

use anyhow::{Context, Result};
use ast_grep_config::{GlobalRules, from_yaml_string};
use ast_grep_language::SupportLang;

use super::{
    SourceFile,
    ast::SyntaxNode,
    language::{
        Decision, Import, Language, Logic, Otherwise, Syntax, Wording, named_entries, standalone,
        unparenthesized,
    },
};
use crate::{config::Policy, report::Report};

pub struct EcmaScript;

pub static ECMASCRIPT: EcmaScript = EcmaScript;

static WORDING: Wording = Wording {
    cross_directory_import: "Use a project alias for cross-directory imports and re-exports.",
    spread_newline: "Put each entry on its own line in an object with a spread and at least three other properties.",
};

fn is_statement(node: &SyntaxNode<'_>) -> bool {
    let kind = node.kind();
    kind.ends_with("_statement") || kind.ends_with("_declaration")
}

impl Syntax for EcmaScript {
    fn wording(&self) -> &'static Wording {
        &WORDING
    }

    fn comment_body<'s>(&self, comment: &'s str) -> Option<(usize, &'s str)> {
        comment
            .strip_prefix("//")
            .or_else(|| comment.strip_prefix("/*")?.strip_suffix("*/"))
            .map(|body| (2, body))
    }

    fn bypass_target(&self, comment: &SyntaxNode<'_>, source: &str) -> Option<Range<usize>> {
        if !standalone(comment, source)
            || !comment.parent().is_some_and(|parent| {
                matches!(
                    parent.kind().as_ref(),
                    "program" | "statement_block" | "switch_case" | "switch_default"
                )
            })
        {
            return None;
        }
        comment
            .next_all()
            .find(|next| next.is_named() && next.kind() != "comment")
            .filter(is_statement)
            .map(|next| next.range())
    }

    fn statements<'t>(&self, node: &SyntaxNode<'t>) -> Option<Vec<SyntaxNode<'t>>> {
        match node.kind().as_ref() {
            "program" | "statement_block" => Some(named_entries(node)),
            "switch_case" | "switch_default" => Some(
                node.field_children("body")
                    .filter(|child| child.kind() != "comment")
                    .collect(),
            ),
            _ => None,
        }
    }

    fn is_function(&self, node: &SyntaxNode<'_>) -> bool {
        matches!(
            node.kind().as_ref(),
            "function_declaration"
                | "function_expression"
                | "generator_function"
                | "generator_function_declaration"
                | "arrow_function"
                | "method_definition"
        )
    }

    fn is_nesting(&self, node: &SyntaxNode<'_>) -> bool {
        // An `else if` continues its chain instead of nesting.
        if node.kind() == "if_statement"
            && node
                .parent()
                .is_some_and(|parent| parent.kind() == "else_clause")
        {
            return false;
        }
        matches!(
            node.kind().as_ref(),
            "if_statement"
                | "for_statement"
                | "for_in_statement"
                | "while_statement"
                | "do_statement"
                | "switch_statement"
                | "try_statement"
                | "with_statement"
        )
    }

    fn is_branch(&self, node: &SyntaxNode<'_>) -> bool {
        matches!(
            node.kind().as_ref(),
            "if_statement"
                | "for_statement"
                | "for_in_statement"
                | "while_statement"
                | "do_statement"
                | "ternary_expression"
                | "switch_case"
                | "catch_clause"
                | "assignment_pattern"
                | "optional_chain"
        ) || node.field("operator").is_some_and(|operator| {
            matches!(
                operator.text().as_ref(),
                "&&" | "||" | "??" | "&&=" | "||=" | "??="
            )
        })
    }

    fn decision<'t>(&self, node: &SyntaxNode<'t>) -> Option<Decision<'t>> {
        if node.kind() != "if_statement" {
            return None;
        }
        let otherwise = match node.field("alternative").and_then(|clause| {
            clause
                .named_children()
                .find(|child| child.kind() != "comment")
        }) {
            None => Otherwise::Nothing,
            Some(statement) if statement.kind() == "if_statement" => Otherwise::ElseIf,
            Some(statement) => Otherwise::Else(statement),
        };
        Some(Decision {
            offset: node.range().start,
            condition: unparenthesized(node.field("condition")?),
            body: node.field("consequence")?,
            otherwise,
        })
    }

    fn logical<'t>(
        &self,
        node: &SyntaxNode<'t>,
    ) -> Option<(Logic, SyntaxNode<'t>, SyntaxNode<'t>)> {
        if node.kind() != "binary_expression" {
            return None;
        }
        let logic = match node.field("operator")?.text().as_ref() {
            "&&" => Logic::And,
            "||" => Logic::Or,
            _ => return None,
        };
        Some((logic, node.field("left")?, node.field("right")?))
    }

    fn negated<'t>(&self, node: &SyntaxNode<'t>) -> Option<SyntaxNode<'t>> {
        (node.kind() == "unary_expression" && node.field("operator")?.text() == "!")
            .then(|| node.field("argument"))
            .flatten()
    }

    fn null_test<'t>(&self, node: &SyntaxNode<'t>) -> Option<SyntaxNode<'t>> {
        if node.kind() != "binary_expression"
            || node.field("operator")?.text() != "==="
            || unparenthesized(node.field("right")?).kind() != "null"
        {
            return None;
        }
        Some(unparenthesized(node.field("left")?)).filter(|left| left.kind() == "identifier")
    }

    fn assignment<'t>(
        &self,
        statement: &SyntaxNode<'t>,
    ) -> Option<(SyntaxNode<'t>, SyntaxNode<'t>)> {
        let statement = if statement.kind() == "statement_block" {
            let entries = named_entries(statement);
            let [only] = entries.as_slice() else {
                return None;
            };
            only.clone()
        } else {
            statement.clone()
        };
        if statement.kind() != "expression_statement" {
            return None;
        }
        let expression = unparenthesized(
            statement
                .named_children()
                .find(|child| child.kind() != "comment")?,
        );
        if expression.kind() != "assignment_expression" {
            return None;
        }
        Some((expression.field("left")?, expression.field("right")?))
    }

    fn is_conditional_expression(&self, node: &SyntaxNode<'_>) -> bool {
        node.kind() == "ternary_expression"
    }

    fn literal_entries<'t>(&self, node: &SyntaxNode<'t>) -> Option<(Vec<SyntaxNode<'t>>, usize)> {
        if node.kind() != "object" {
            return None;
        }
        let entries = named_entries(node);
        let spreads = entries
            .iter()
            .filter(|entry| entry.kind() == "spread_element")
            .count();
        Some((entries, spreads))
    }

    fn imports(&self, root: &SyntaxNode<'_>) -> Vec<Import> {
        let mut imports = Vec::new();
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
            let specifier = source_node.text();
            let path = specifier[1..specifier.len() - 1].replace("\\\\", "/");
            imports.push(Import {
                offset: source_node.range().start,
                relative: path.starts_with('.'),
                path,
                members: Vec::new(),
            });
        }
        imports
    }

    fn module_files(&self, base: &str, _members: &[String]) -> Vec<String> {
        let alternatives: &[(&str, &[&str])] = &[
            (".js", &[".ts", ".tsx", ".jsx"]),
            (".mjs", &[".mts"]),
            (".cjs", &[".cts"]),
        ];
        let mut files = vec![base.to_owned()];
        for (extension, replacements) in alternatives {
            if let Some(stem) = base.strip_suffix(extension) {
                files.extend(
                    replacements
                        .iter()
                        .map(|extension| format!("{stem}{extension}")),
                );
                return files;
            }
        }
        files.extend(
            [
                ".ts",
                ".tsx",
                ".js",
                ".jsx",
                "/index.ts",
                "/index.tsx",
                "/index.js",
                "/index.jsx",
            ]
            .into_iter()
            .map(|suffix| format!("{base}{suffix}")),
        );
        files
    }

    fn loop_body<'t>(&self, node: &SyntaxNode<'t>) -> Option<SyntaxNode<'t>> {
        matches!(
            node.kind().as_ref(),
            "for_statement" | "for_in_statement" | "while_statement" | "do_statement"
        )
        .then(|| node.field("body"))
        .flatten()
    }

    fn skips_token(&self, kind: &str) -> bool {
        matches!(kind, "comment" | ";" | "type_annotation")
    }
}

fn emit(
    node: &SyntaxNode<'_>,
    input: &SourceFile<'_>,
    policy: &Policy<'_>,
    rule: &str,
    message: &str,
    report: &mut Report,
) {
    if let Some(severity) = policy.severity(rule) {
        report.finding(
            input.path,
            input.text,
            node.range().start,
            severity,
            rule,
            message,
        );
    }
}

fn check_exports(
    root: &SyntaxNode<'_>,
    input: &SourceFile<'_>,
    policy: &Policy<'_>,
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
                    input,
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
                input,
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
                input,
                policy,
                "no-restricted-syntax",
                "Collect type exports in a separate export type list.",
                report,
            );
        }
    }
}

/// Embedded ast-grep rules and export organization, which only JS/TS define.
pub fn check_structure(
    root: &SyntaxNode<'_>,
    language: Language,
    input: &SourceFile<'_>,
    policy: &Policy<'_>,
    report: &mut Report,
) -> Result<()> {
    let label = match language {
        Language::Tsx => "Tsx",
        Language::TypeScript => "TypeScript",
        _ => "JavaScript",
    };
    let ast_rules = from_yaml_string::<SupportLang>(
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
    for rule in ast_rules {
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
            emit(&matched, input, policy, &rule.id, &rule.message, report);
        }
    }
    if policy.severity("no-restricted-syntax").is_some() {
        check_exports(root, input, policy, report);
    }
    Ok(())
}

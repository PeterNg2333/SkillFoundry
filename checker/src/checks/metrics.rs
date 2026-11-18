use super::SourceFile;
use super::ast::{self, SyntaxNode};
use crate::{config::Policy, report::Report};

fn function(node: &SyntaxNode<'_>) -> bool {
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

fn control(node: &SyntaxNode<'_>) -> bool {
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

fn branches(node: &SyntaxNode<'_>) -> usize {
    if function(node) {
        return 0;
    }
    let branch = matches!(
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
    });
    usize::from(branch)
        + node
            .named_children()
            .map(|child| branches(&child))
            .sum::<usize>()
}

fn warn(
    policy: &Policy<'_>,
    input: &SourceFile<'_>,
    offset: usize,
    rule: &str,
    value: usize,
    limit: u64,
    report: &mut Report,
) {
    if value as u64 > limit
        && let Some(severity) = policy.severity(rule)
    {
        report.finding(
            input.path,
            input.text,
            offset,
            severity,
            rule,
            format!("{rule} is {value}; maximum allowed is {limit}."),
        );
    }
}

pub fn check(
    root: &SyntaxNode<'_>,
    input: &SourceFile<'_>,
    policy: &Policy<'_>,
    report: &mut Report,
) {
    let file = input.path;
    let source = input.text;
    let mut max_depth = 0;
    let mut max_complexity = 0;
    for node in root.dfs() {
        if control(&node) {
            let depth = 1 + node
                .ancestors()
                .take_while(|ancestor| !function(ancestor))
                .filter(control)
                .count();
            max_depth = max_depth.max(depth);
            warn(
                policy,
                input,
                node.range().start,
                "max-depth",
                depth,
                policy.threshold("max-depth", 2),
                report,
            );
        }
        if function(&node) {
            let complexity = 1 + node
                .named_children()
                .map(|child| branches(&child))
                .sum::<usize>();
            max_complexity = max_complexity.max(complexity);
            warn(
                policy,
                input,
                node.range().start,
                "complexity",
                complexity,
                policy.threshold("complexity", 10),
                report,
            );
        }
    }
    let options = policy.option("max-lines");
    let skip_comments = options
        .and_then(|options| options.get("skipComments"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let skip_blank = options
        .and_then(|options| options.get("skipBlankLines"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let (_, stripped) = ast::comments(root, source);
    let lines = source
        .lines()
        .zip(stripped.lines())
        .filter(|(original, stripped)| {
            !(skip_blank && original.trim().is_empty()
                || skip_comments && !original.trim().is_empty() && stripped.trim().is_empty())
        })
        .count();
    let limit = options
        .and_then(|options| {
            options
                .as_u64()
                .or_else(|| options.get("max").and_then(|value| value.as_u64()))
        })
        .unwrap_or(300);
    warn(policy, input, 0, "max-lines", lines, limit, report);
    report.metric(file, "structure", "maxDepth", max_depth);
    report.metric(file, "structure", "maxComplexity", max_complexity);
    report.metric(file, "structure", "sourceLines", lines);
}

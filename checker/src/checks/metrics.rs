//! `max-depth`, `complexity` and `max-lines` with per-language structure roles.

use super::{
    SourceFile,
    ast::{self, SyntaxNode},
    language::Syntax,
};
use crate::{config::Policy, report::Report};

fn branches(syntax: &dyn Syntax, node: &SyntaxNode<'_>) -> usize {
    if syntax.is_function(node) {
        return 0;
    }
    usize::from(syntax.is_branch(node))
        + node
            .named_children()
            .map(|child| branches(syntax, &child))
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
    syntax: &dyn Syntax,
    policy: &Policy<'_>,
    report: &mut Report,
) {
    let file = input.path;
    let source = input.text;
    let mut max_depth = 0;
    let mut max_complexity = 0;
    for node in root.dfs() {
        if syntax.is_nesting(&node) {
            let depth = 1 + node
                .ancestors()
                .take_while(|ancestor| !syntax.is_function(ancestor))
                .filter(|ancestor| syntax.is_nesting(ancestor))
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
        if syntax.is_function(&node) {
            let complexity = 1 + node
                .named_children()
                .map(|child| branches(syntax, &child))
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

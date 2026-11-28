//! `maintainable/long-condition` and `maintainable/fallback-order` over language decisions.

use super::{
    SourceFile,
    ast::SyntaxNode,
    language::{Logic, Otherwise, Syntax, intact, unparenthesized},
};
use crate::{
    catalog::{FALLBACK_ORDER, LONG_CONDITION},
    config::Policy,
    report::Report,
};

/// Terms combined by `and`, `or` and negation. Nullish coalescing counts as one term.
fn condition_terms(syntax: &dyn Syntax, condition: SyntaxNode<'_>) -> usize {
    let condition = unparenthesized(condition);
    if let Some((_, left, right)) = syntax.logical(&condition) {
        return condition_terms(syntax, left) + condition_terms(syntax, right);
    }
    syntax
        .negated(&condition)
        .map_or(1, |inner| condition_terms(syntax, inner))
}

/// The variable a statement assigns when its leftmost `and` term finds it null.
fn null_guard_target(syntax: &dyn Syntax, statement: &SyntaxNode<'_>) -> Option<String> {
    let decision = syntax.decision(statement)?;
    if !matches!(decision.otherwise, Otherwise::Nothing) || !intact(statement) {
        return None;
    }
    let (Logic::And, mut guard, _) = syntax.logical(&decision.condition)? else {
        return None;
    };
    guard = unparenthesized(guard);
    while let Some((Logic::And, left, _)) = syntax.logical(&guard) {
        guard = unparenthesized(left);
    }
    let read = syntax.null_test(&guard)?;
    let (write, _) = syntax.assignment(&decision.body)?;
    (write.kind() == "identifier" && read.text() == write.text()).then(|| write.text().into_owned())
}

pub fn check(
    root: &SyntaxNode<'_>,
    input: &SourceFile<'_>,
    syntax: &dyn Syntax,
    policy: &Policy<'_>,
    report: &mut Report,
) {
    let long_condition = policy.severity(LONG_CONDITION);
    let fallback_order = policy.severity(FALLBACK_ORDER);
    if long_condition.is_none() && fallback_order.is_none() {
        return;
    }
    for node in root.dfs().filter(|node| node.is_named()) {
        if let Some(decision) = syntax.decision(&node)
            && intact(&decision.condition)
        {
            if let Some(severity) = long_condition {
                let terms = condition_terms(syntax, decision.condition.clone());
                if terms >= 4 {
                    report.finding(
                        input.path,
                        input.text,
                        decision.offset,
                        severity,
                        LONG_CONDITION,
                        format!("Split this if condition into smaller decisions. It combines {terms} terms."),
                    );
                }
            }
            if let Some(severity) = fallback_order
                && let Otherwise::Else(statement) = &decision.otherwise
                && intact(statement)
                && let Some((_, value)) = syntax.assignment(statement)
                && syntax.is_conditional_expression(&unparenthesized(value))
            {
                report.finding(
                    input.path,
                    input.text,
                    decision.offset,
                    severity,
                    FALLBACK_ORDER,
                    "Make the fallback decision explicit instead of assigning a ternary in the else branch.",
                );
            }
        }
        let Some(severity) = fallback_order else {
            continue;
        };
        let Some(statements) = syntax.statements(&node) else {
            continue;
        };
        for pair in statements.windows(2) {
            let Some(target) = null_guard_target(syntax, &pair[1]) else {
                continue;
            };
            if null_guard_target(syntax, &pair[0]).as_ref() == Some(&target) {
                report.finding(
                    input.path,
                    input.text,
                    pair[1].range().start,
                    severity,
                    FALLBACK_ORDER,
                    format!("Choose the fallback for {target} in priority order instead of repeatedly checking and assigning it."),
                );
            }
        }
    }
}

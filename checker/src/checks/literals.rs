//! `maintainable/object-spread-newline` for object and dictionary literals.

use super::{
    SourceFile,
    ast::SyntaxNode,
    language::{Syntax, intact},
};
use crate::{catalog::OBJECT_SPREAD_NEWLINE, config::Policy, report::Report};

pub fn check(
    root: &SyntaxNode<'_>,
    input: &SourceFile<'_>,
    syntax: &dyn Syntax,
    policy: &Policy<'_>,
    report: &mut Report,
) {
    let Some(severity) = policy.severity(OBJECT_SPREAD_NEWLINE) else {
        return;
    };
    for node in root.dfs() {
        let Some((entries, spreads)) = syntax.literal_entries(&node) else {
            continue;
        };
        if entries.len() < 4 || spreads == 0 || entries.len() - spreads < 3 || !intact(&node) {
            continue;
        }
        let shares_line = entries.windows(2).any(|pair| {
            !input.text[pair[0].range().end..pair[1].range().start]
                .contains(['\r', '\n', '\u{2028}', '\u{2029}'])
        });
        if shares_line {
            report.finding(
                input.path,
                input.text,
                node.range().start,
                severity,
                OBJECT_SPREAD_NEWLINE,
                syntax.wording().spread_newline,
            );
        }
    }
}

use std::ops::Range;

use super::{SourceFile, ast::SyntaxNode};
use crate::{
    config::{Policy, RULES, SIMILAR_LOOPS, WORK_TAGS, WorkDirectives},
    report::{Bypass, Directive, Finding, Report, source_location},
};

struct LocalBypass<'a> {
    offset: usize,
    target: Range<usize>,
    rule: &'a str,
    reason: &'a str,
    used: bool,
}

pub struct LoopBypass {
    pub target: Range<usize>,
    pub line: usize,
    pub reason: String,
    pub used: bool,
    offset: usize,
    column: usize,
}

impl LoopBypass {
    pub fn finish(self, file: &str, report: &mut Report) {
        if !self.used {
            report.diagnostics.push(Finding {
                file: file.to_owned(),
                line: self.line,
                column: self.column,
                severity: "error".to_owned(),
                rule: "maintainable/unused-bypass".to_owned(),
                message: format!("Remove the unused @bypass for {SIMILAR_LOOPS}."),
                origin: "static",
                related: None,
                offset: self.offset,
            });
        }
    }
}

fn bypass_target(node: &SyntaxNode<'_>, source: &str) -> Option<Range<usize>> {
    let line_start = source[..node.range().start]
        .rfind('\n')
        .map_or(0, |end| end + 1);
    if !source[line_start..node.range().start].trim().is_empty()
        || !node.parent().is_some_and(|parent| {
            matches!(
                parent.kind().as_ref(),
                "program" | "statement_block" | "switch_case" | "switch_default"
            )
        })
    {
        return None;
    }
    node.next_all()
        .find(|next| next.is_named() && next.kind() != "comment")
        .filter(|next| {
            let kind = next.kind();
            kind.ends_with("_statement") || kind.ends_with("_declaration")
        })
        .map(|next| next.range())
}

fn record_work(
    input: (&str, &str, usize),
    (tag, text): (&str, &str),
    work: &WorkDirectives,
    report: &mut Report,
) {
    let Some(name) = tag.strip_prefix('@') else {
        return;
    };
    let Some(&kind) = WORK_TAGS.iter().find(|&&kind| kind == name) else {
        return;
    };
    if work
        .tags
        .as_ref()
        .is_some_and(|tags| !tags.iter().any(|tag| tag == kind))
    {
        return;
    }
    let (file, source, offset) = input;
    let text = text.trim();
    if text.is_empty() {
        report.finding(
            file,
            source,
            offset,
            "error",
            "maintainable/invalid-directive",
            format!("Provide a description after {tag}."),
        );
        return;
    }
    let (line, column) = source_location(source, offset);
    report.directives.push(Directive {
        file: file.to_owned(),
        line,
        column,
        kind,
        text: text.to_owned(),
    });
}

pub fn check_spec(file: &str, source: &str, work: &WorkDirectives, report: &mut Report) {
    let mut in_work = false;
    let mut fence = None;
    let mut offset = 0;
    for raw in source.split_inclusive('\n') {
        let start = offset;
        offset += raw.len();
        let line = raw.trim_end();
        let trimmed = line.trim_start_matches(' ');
        let indent = line.len() - trimmed.len();
        if indent > 3 || trimmed.starts_with('\t') {
            continue;
        }
        if let Some(&marker @ (b'`' | b'~')) = trimmed.as_bytes().first() {
            let width = trimmed.bytes().take_while(|byte| *byte == marker).count();
            if width >= 3 {
                match fence {
                    None => fence = Some((marker, width)),
                    Some((open, minimum))
                        if marker == open && width >= minimum && trimmed[width..].is_empty() =>
                    {
                        fence = None;
                    }
                    _ => {}
                }
                continue;
            }
        }
        if fence.is_some() {
            continue;
        }
        let (tag, text) = trimmed
            .split_once(char::is_whitespace)
            .unwrap_or((trimmed, ""));
        if matches!(tag, "#" | "##") {
            in_work = tag == "##" && text.trim() == "Work";
        } else if in_work {
            record_work((file, source, start + indent), (tag, text), work, report);
        }
    }
}

pub fn check(
    root: &SyntaxNode<'_>,
    input: &SourceFile<'_>,
    style_policy: Option<&Policy<'_>>,
    work: Option<&WorkDirectives>,
    first_finding: usize,
    report: &mut Report,
) -> Vec<LoopBypass> {
    let mut bypasses = Vec::new();
    let mut loop_bypasses = Vec::new();
    for node in root.dfs().filter(|node| node.kind() == "comment") {
        let range = node.range();
        let comment = &input.text[range.clone()];
        let Some(body) = comment
            .strip_prefix("//")
            .or_else(|| comment.strip_prefix("/*")?.strip_suffix("*/"))
        else {
            continue;
        };
        let mut line_offset = range.start + 2;
        for line in body.split_inclusive('\n') {
            let trimmed = line.trim_start();
            let tag = trimmed.strip_prefix('*').unwrap_or(trimmed).trim_start();
            let offset = line_offset + line.len() - tag.len();
            line_offset += line.len();
            let (kind, text) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
            let text = text.trim();
            match kind {
                "@bypass" if style_policy.is_some() => {
                    let details = text
                        .split_once("--")
                        .map(|(rule, reason)| (rule.trim(), reason.trim()));
                    if let Some((rule, reason)) = details
                        && RULES.contains(&rule)
                        && !reason.is_empty()
                        && let Some(target) = bypass_target(&node, input.text)
                    {
                        if rule == SIMILAR_LOOPS {
                            if style_policy.is_some_and(|policy| policy.severity(rule).is_some()) {
                                let (line, column) = source_location(input.text, offset);
                                loop_bypasses.push(LoopBypass {
                                    target,
                                    line,
                                    column,
                                    reason: reason.to_owned(),
                                    used: false,
                                    offset,
                                });
                            }
                            continue;
                        }
                        bypasses.push(LocalBypass {
                            offset,
                            target,
                            rule,
                            reason,
                            used: false,
                        });
                    } else {
                        report.finding(input.path, input.text, offset, "error", "maintainable/invalid-bypass",
                            "Place @bypass <rule-id> -- <reason> on its own comment before a statement or declaration.");
                    }
                }
                _ if let Some(work) = work => {
                    record_work((input.path, input.text, offset), (kind, text), work, report);
                }
                _ => {}
            }
        }
    }

    if let Some(policy) = style_policy.filter(|_| !bypasses.is_empty()) {
        let findings = report.diagnostics.split_off(first_finding);
        for finding in findings {
            if let Some(bypass) = bypasses.iter_mut().find(|bypass| {
                bypass.rule == finding.rule && bypass.target.contains(&finding.offset)
            }) {
                bypass.used = true;
                report.bypasses.push(Bypass {
                    finding,
                    directive_line: Some(source_location(input.text, bypass.offset).0),
                    reason: bypass.reason.to_owned(),
                });
            } else {
                report.diagnostics.push(finding);
            }
        }
        for bypass in bypasses {
            if !bypass.used && policy.severity(bypass.rule).is_some() {
                report.finding(
                    input.path,
                    input.text,
                    bypass.offset,
                    "error",
                    "maintainable/unused-bypass",
                    format!("Remove the unused @bypass for {}.", bypass.rule),
                );
            }
        }
    }
    loop_bypasses
}

//! The per-file pipeline for languages behind the shared rule contract.
//!
//! Each step names its owner: the language adapter validates syntax and supplies facts, the
//! JS/TS adapter adds its language-only rules, and the shared rule modules run for every
//! language through [`Syntax`](super::language::Syntax).

use anyhow::Result;

use super::{
    SourceFile, ast, decisions, directives, helpers, imports, javascript, language::Language,
    literals, loop_similarity, metrics, python, semantics,
};
use crate::{
    catalog::{CROSS_DIRECTORY_IMPORTS, SIMILAR_LOOPS, SINGLE_USE_HELPER, TYPESCRIPT},
    config::{Policy, Settings},
    report::Report,
};

/// Which parts of the analysis the selected checks request for this file.
pub struct Scope<'a> {
    pub inspect: bool,
    pub comments: bool,
    pub directives: bool,
    pub rules: &'a [String],
}

pub fn check(
    input: &SourceFile<'_>,
    language: Language,
    settings: &Settings,
    scope: &Scope<'_>,
    report: &mut Report,
) -> Result<Option<loop_similarity::File>> {
    let rules = scope.rules;
    let policy = Policy::for_file(settings, input.path, language.kind(), rules)?;
    let syntax = language.syntax();
    let first_finding = report.diagnostics.len();
    let collect_loops = scope.inspect && loop_similarity::requested(settings, rules);
    let tree = ast::parse(input.text, language.grammar())?;
    let root = tree.root();
    let facts = match language {
        Language::Python => {
            python::facts(&root, input, scope.inspect, &policy, collect_loops, report)
        }
        _ => semantics::check(
            input.path,
            input.text,
            input.kind == TYPESCRIPT,
            scope.inspect,
            &policy,
            collect_loops,
            report,
        ),
    };
    let Some(facts) = facts else {
        return Ok(None);
    };
    let wants = |names: &[&str]| {
        rules.is_empty() || rules.iter().any(|rule| names.contains(&rule.as_str()))
    };
    if scope.inspect {
        if language != Language::Python {
            javascript::check_structure(&root, language, input, &policy, report)?;
        }
        if wants(&[CROSS_DIRECTORY_IMPORTS]) {
            imports::check(&root, input, syntax, (settings, &policy, rules), report);
        }
        if wants(&["max-depth", "complexity", "max-lines"]) {
            metrics::check(&root, input, syntax, &policy, report);
        }
        decisions::check(&root, input, syntax, &policy, report);
        literals::check(&root, input, syntax, &policy, report);
        if let Some(severity) = policy.severity(SINGLE_USE_HELPER) {
            helpers::check(&facts.helpers, input.path, input.text, severity, report);
        }
    }
    if scope.comments {
        let (groups, _) = ast::comments(&root, input.text);
        ast::check_comments(input.path, input.text, groups, &settings.limits, report);
    }
    let bypasses = if scope.inspect || scope.directives {
        directives::check(
            &root,
            input,
            syntax,
            scope.inspect.then_some(&policy),
            scope.directives.then_some(&settings.work_directives),
            first_finding,
            report,
        )
    } else {
        Vec::new()
    };
    Ok(collect_loops.then(|| {
        loop_similarity::collect(
            &root,
            input,
            language,
            facts,
            policy.severity(SIMILAR_LOOPS),
            settings.loop_similarity,
            bypasses,
        )
    }))
}

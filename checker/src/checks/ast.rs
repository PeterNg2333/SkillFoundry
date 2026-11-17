use anyhow::{Result, anyhow};
use ast_grep_core::{Node, tree_sitter::StrDoc};
use ast_grep_language::SupportLang;

use crate::{config::Limits, report::Report};

pub type SyntaxNode<'a> = Node<'a, StrDoc<SupportLang>>;

pub fn parse(
    source: &str,
    language: SupportLang,
) -> Result<ast_grep_core::AstGrep<StrDoc<SupportLang>>> {
    let document = StrDoc::try_new(source, language).map_err(|error| anyhow!(error))?;
    Ok(ast_grep_core::AstGrep::doc(document))
}

pub fn comments(root: &SyntaxNode<'_>, source: &str) -> (usize, String) {
    let mut stripped = source.as_bytes().to_vec();
    let mut groups = 0;
    let mut previous_end: Option<usize> = None;
    for node in root.dfs().filter(|node| node.kind() == "comment") {
        let range = node.range();
        if !previous_end.is_some_and(|end| {
            source[end..range.start].chars().all(char::is_whitespace)
                && source[end..range.start]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    <= 1
        }) {
            groups += 1;
        }
        previous_end = Some(range.end);
        for byte in &mut stripped[range] {
            if !matches!(*byte, b'\r' | b'\n') {
                *byte = b' ';
            }
        }
    }
    (
        groups,
        String::from_utf8(stripped).expect("comment replacement preserves UTF-8"),
    )
}

pub fn check_go(
    file: &str,
    source: &str,
    limits: &Limits,
    inspect: bool,
    inspect_comments: bool,
    report: &mut Report,
) -> Result<()> {
    let tree = parse(source, SupportLang::Go)?;
    let root = tree.root();
    if let Some(node) = root.dfs().find(|node| node.is_error() || node.is_missing()) {
        report.finding(
            file,
            source,
            node.range().start,
            "error",
            "go/syntax",
            "Invalid Go syntax.",
        );
        return Ok(());
    }
    if inspect {
        let imports = root
            .dfs()
            .filter(|node| node.kind() == "import_spec")
            .count();
        let functions = root
            .dfs()
            .filter(|node| {
                matches!(
                    node.kind().as_ref(),
                    "function_declaration" | "method_declaration"
                )
            })
            .count();
        report.metric(file, "go", "imports", imports);
        report.metric(file, "go", "functionDeclarations", functions);
        if limits.imports_per_file.is_some_and(|limit| imports > limit) {
            report.finding(
                file,
                source,
                0,
                "warning",
                "imports/count",
                format!("{imports} imports exceed the configured limit."),
            );
        }
    }
    if inspect_comments {
        let (groups, _) = comments(&root, source);
        check_comments(file, source, groups, limits, report);
    }
    Ok(())
}

pub fn check_comments(
    file: &str,
    source: &str,
    groups: usize,
    limits: &Limits,
    report: &mut Report,
) {
    report.metric(file, "comments", "commentGroups", groups);
    if limits.require_comments && groups == 0 {
        report.finding(
            file,
            source,
            0,
            "warning",
            "comments/presence",
            "The configured policy requires a comment.",
        );
    }
}

//! `maintainable/cross-directory-imports`, per-file import counts and the import limit.

use std::path::Path;

use super::{SourceFile, ast::SyntaxNode, language::Syntax};
use crate::{
    catalog::CROSS_DIRECTORY_IMPORTS,
    config::{Policy, Settings},
    report::Report,
    selection::normalized,
};

pub fn check(
    root: &SyntaxNode<'_>,
    input: &SourceFile<'_>,
    syntax: &dyn Syntax,
    (settings, policy, rules): (&Settings, &Policy<'_>, &[String]),
    report: &mut Report,
) {
    let imports = syntax.imports(root);
    if let Some(severity) = policy.severity(CROSS_DIRECTORY_IMPORTS) {
        let directory = input
            .project
            .join(input.path)
            .parent()
            .unwrap()
            .to_path_buf();
        for import in imports.iter().filter(|import| import.relative) {
            if matches!(import.path.as_str(), "." | "./") {
                continue;
            }
            let destination = normalized(&directory.join(Path::new(&import.path)));
            if destination.parent() != Some(directory.as_path()) || destination.is_dir() {
                report.finding(
                    input.path,
                    input.text,
                    import.offset,
                    severity,
                    CROSS_DIRECTORY_IMPORTS,
                    syntax.wording().cross_directory_import,
                );
            }
        }
    }
    report.metric(input.path, input.kind, "imports", imports.len());
    if rules.is_empty()
        && settings
            .limits
            .imports_per_file
            .is_some_and(|limit| imports.len() > limit)
    {
        report.finding(
            input.path,
            input.text,
            0,
            "warning",
            "imports/count",
            format!("{} imports exceed the configured limit.", imports.len()),
        );
    }
}

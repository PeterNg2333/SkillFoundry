use oxc_allocator::Allocator;
use oxc_ast::{
    AstKind,
    ast::{ImportDeclarationSpecifier, VariableDeclarationKind},
};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;

use crate::{config::Policy, report::Report};

pub fn check(
    file: &str,
    source: &str,
    typescript: bool,
    inspect: bool,
    policy: &Policy,
    report: &mut Report,
) -> bool {
    let allocator = Allocator::default();
    let extension = file.rsplit('.').next().unwrap_or("").to_lowercase();
    let mut source_type = if typescript {
        SourceType::ts()
    } else {
        SourceType::mjs()
    };
    source_type = source_type.with_jsx(matches!(extension.as_str(), "jsx" | "tsx"));
    if matches!(extension.as_str(), "cjs" | "cts") {
        source_type = source_type.with_module(false);
    }
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        for error in parsed.diagnostics {
            let offset = error
                .labels
                .first()
                .map_or(0, |label| label.offset() as usize);
            report.finding(file, source, offset, "error", "syntax", error.to_string());
        }
        return false;
    }
    let built = SemanticBuilder::new()
        .with_build_nodes(true)
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.diagnostics.is_empty() {
        for error in built.diagnostics {
            let offset = error
                .labels
                .first()
                .map_or(0, |label| label.offset() as usize);
            report.finding(file, source, offset, "error", "syntax", error.to_string());
        }
        return false;
    }
    if !inspect {
        return true;
    }
    let semantic = built.semantic;
    let scope = semantic.scoping();
    // Decorator metadata may require a runtime value for a type annotation.
    let has_decorators = semantic
        .nodes()
        .iter()
        .any(|node| matches!(node.kind(), AstKind::Decorator(_)));
    for node in semantic.nodes().iter() {
        match node.kind() {
            AstKind::VariableDeclaration(declaration)
                if declaration.kind == VariableDeclarationKind::Let =>
            {
                let Some(severity) = policy.severity("prefer-const") else {
                    continue;
                };
                for declarator in &declaration.declarations {
                    if declarator.init.is_none() {
                        continue;
                    }
                    for binding in declarator.id.get_binding_identifiers() {
                        if binding
                            .symbol_id
                            .get()
                            .is_some_and(|symbol| !scope.symbol_is_mutated(symbol))
                        {
                            report.finding(
                                file,
                                source,
                                binding.span.start as usize,
                                severity,
                                "prefer-const",
                                format!("{} is never reassigned; use const.", binding.name),
                            );
                        }
                    }
                }
            }
            AstKind::ImportDeclaration(declaration)
                if typescript && !has_decorators && !declaration.import_kind.is_type() =>
            {
                let Some(severity) = policy.severity("@typescript-eslint/consistent-type-imports")
                else {
                    continue;
                };
                let Some(specifiers) = &declaration.specifiers else {
                    continue;
                };
                for specifier in specifiers {
                    if matches!(specifier, ImportDeclarationSpecifier::ImportSpecifier(specifier) if specifier.import_kind.is_type())
                    {
                        continue;
                    }
                    let binding = specifier.local();
                    let Some(symbol) = binding.symbol_id.get() else {
                        continue;
                    };
                    let references: Vec<_> = scope.get_resolved_references(symbol).collect();
                    if !references.is_empty()
                        && references
                            .iter()
                            .all(|reference| reference.is_type() && !reference.is_value())
                    {
                        report.finding(
                            file,
                            source,
                            binding.span.start as usize,
                            severity,
                            "@typescript-eslint/consistent-type-imports",
                            format!("{} is used only as a type; use import type.", binding.name),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    true
}

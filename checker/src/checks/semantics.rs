//! Oxc syntax validation and binding analysis for JavaScript and TypeScript.
//!
//! Oxc supplies JS/TS [`Facts`] for the shared rules and runs the JS/TS-only semantic rules.

use oxc_allocator::Allocator;
use oxc_ast::{
    AstKind,
    ast::{Expression, ImportDeclarationSpecifier, VariableDeclarationKind},
};
use oxc_parser::Parser;
use oxc_semantic::{AstNode, Semantic, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};

use super::{
    helpers::{Helper, Reference},
    language::{Facts, Import},
};
use crate::{
    catalog::{SIMILAR_LOOPS, SINGLE_USE_HELPER, TYPE_IMPORTS},
    config::Policy,
    report::Report,
};

fn runtime_import(kind: AstKind<'_>) -> Option<&str> {
    match kind {
        AstKind::ImportDeclaration(import)
            if !import.import_kind.is_type()
                && import.specifiers.as_ref().is_none_or(|items| {
                    items.is_empty()
                        || items.iter().any(|item| {
                            !matches!(item,
                    ImportDeclarationSpecifier::ImportSpecifier(item) if item.import_kind.is_type())
                        })
                }) =>
        {
            Some(import.source.value.as_str())
        }
        AstKind::ExportFromDeclaration(export)
            if !export.export_kind.is_type()
                && (export.specifiers.is_empty()
                    || export
                        .specifiers
                        .iter()
                        .any(|item| !item.export_kind.is_type())) =>
        {
            Some(export.source.value.as_str())
        }
        AstKind::ExportAllDeclaration(export) if !export.export_kind.is_type() => {
            Some(export.source.value.as_str())
        }
        AstKind::ImportExpression(import) => match &import.source {
            Expression::StringLiteral(source) => Some(source.value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn single_use_candidate(node: &AstNode<'_>, semantic: &Semantic<'_>) -> Option<Helper> {
    let nodes = semantic.nodes();
    let (binding, declaration_id) = match node.kind() {
        AstKind::VariableDeclarator(declaration) => {
            let parent_id = nodes.parent_id(node.id());
            let is_const = matches!(nodes.kind(parent_id), AstKind::VariableDeclaration(parent)
                if parent.kind == VariableDeclarationKind::Const);
            let is_function = match declaration.init.as_ref() {
                Some(Expression::ArrowFunctionExpression(_)) => true,
                Some(Expression::FunctionExpression(function)) => !function.generator,
                _ => false,
            };
            if !is_const || !is_function {
                return None;
            }
            (declaration.id.get_binding_identifier(), parent_id)
        }
        AstKind::Function(function)
            if function.is_declaration() && function.body.is_some() && !function.generator =>
        {
            (function.id.as_ref(), node.id())
        }
        _ => return None,
    };
    if matches!(
        nodes.parent_kind(declaration_id),
        AstKind::ExportDeclaration(_) | AstKind::ExportDefaultDeclaration(_)
    ) {
        return None;
    }
    let binding = binding?;
    let symbol = binding.symbol_id.get()?;
    let scope = semantic.scoping();
    if !scope.symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    let references = scope
        .get_resolved_references(symbol)
        .map(|reference| {
            let reference_id = reference.node_id();
            let reference_span = nodes.kind(reference_id).span();
            Reference {
                offset: reference_span.start as usize,
                direct_call: matches!(nodes.parent_kind(reference_id), AstKind::CallExpression(call)
                    if !call.optional && call.callee.span() == reference_span),
            }
        })
        .collect();
    let span = node.span();
    Some(Helper {
        name: binding.name.to_string(),
        offset: binding.span.start as usize,
        declaration: span.start as usize..span.end as usize,
        references,
    })
}

pub fn check(
    file: &str,
    source: &str,
    typescript: bool,
    inspect: bool,
    policy: &Policy<'_>,
    collect_loop_inputs: bool,
    report: &mut Report,
) -> Option<Facts> {
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
        return None;
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
        return None;
    }
    let mut facts = Facts::default();
    if !inspect {
        return Some(facts);
    }
    let semantic = built.semantic;
    super::ui::check(&semantic, file, source, policy, report);
    let scope = semantic.scoping();
    // Decorator metadata may require a runtime value for a type annotation.
    let has_decorators = semantic
        .nodes()
        .iter()
        .any(|node| matches!(node.kind(), AstKind::Decorator(_)));
    let collect_helpers = policy.severity(SINGLE_USE_HELPER).is_some();
    let collect_bindings = collect_loop_inputs && policy.severity(SIMILAR_LOOPS).is_some();
    for node in semantic.nodes().iter() {
        if collect_loop_inputs
            && let Some(specifier) = runtime_import(node.kind())
            && (specifier.starts_with("./") || specifier.starts_with("../"))
        {
            facts.imports.push(Import {
                offset: node.kind().span().start as usize,
                path: specifier.to_owned(),
                relative: true,
                members: Vec::new(),
            });
        }
        if collect_bindings {
            let symbol = match node.kind() {
                AstKind::BindingIdentifier(binding) => binding.symbol_id.get(),
                AstKind::IdentifierReference(reference) => reference
                    .reference_id
                    .get()
                    .and_then(|id| scope.get_reference(id).symbol_id()),
                _ => None,
            };
            if let Some(symbol) = symbol {
                facts.bindings.insert(
                    node.kind().span().start as usize,
                    scope.symbol_span(symbol).start as usize,
                );
            }
        }
        if collect_helpers && let Some(helper) = single_use_candidate(node, &semantic) {
            facts.helpers.push(helper);
        }
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
                let Some(severity) = policy.severity(TYPE_IMPORTS) else {
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
                            TYPE_IMPORTS,
                            format!("{} is used only as a type; use import type.", binding.name),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    Some(facts)
}

use oxc_allocator::Allocator;
use oxc_ast::{
    AstKind,
    ast::{
        AssignmentExpression, AssignmentTarget, Expression, ImportDeclarationSpecifier,
        ObjectPropertyKind, Statement, VariableDeclarationKind,
    },
};
use oxc_parser::Parser;
use oxc_semantic::{AstNode, Semantic, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};

use crate::{
    config::{
        FALLBACK_ORDER, LONG_CONDITION, OBJECT_SPREAD_NEWLINE, Policy, SIMILAR_LOOPS,
        SINGLE_USE_HELPER,
    },
    report::Report,
};

#[derive(Default)]
pub struct LoopInputs {
    pub bindings: BTreeMap<usize, usize>,
    pub imports: Vec<String>,
}

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

fn condition_terms(expression: &Expression<'_>) -> usize {
    match expression.without_parentheses() {
        Expression::LogicalExpression(logical) if !logical.operator.is_coalesce() => {
            condition_terms(&logical.left) + condition_terms(&logical.right)
        }
        Expression::UnaryExpression(unary) if unary.operator.is_not() => {
            condition_terms(&unary.argument)
        }
        _ => 1,
    }
}

fn single_assignment<'s, 'a>(statement: &'s Statement<'a>) -> Option<&'s AssignmentExpression<'a>> {
    let statement = if let Statement::BlockStatement(block) = statement {
        let [only] = block.body.as_slice() else {
            return None;
        };
        only
    } else {
        statement
    };
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = statement.expression.without_parentheses()
    else {
        return None;
    };
    assignment.operator.is_assign().then_some(assignment)
}

fn null_guard_target<'s>(statement: &'s Statement<'_>) -> Option<&'s str> {
    let Statement::IfStatement(statement) = statement else {
        return None;
    };
    if statement.alternate.is_some() {
        return None;
    }
    let Expression::LogicalExpression(condition) = statement.test.without_parentheses() else {
        return None;
    };
    if !condition.operator.is_and() {
        return None;
    }
    let mut guard = condition.left.without_parentheses();
    while let Expression::LogicalExpression(condition) = guard
        && condition.operator.is_and()
    {
        guard = condition.left.without_parentheses();
    }
    let Expression::BinaryExpression(guard) = guard else {
        return None;
    };
    if guard.operator.as_str() != "==="
        || !matches!(
            guard.right.without_parentheses(),
            Expression::NullLiteral(_)
        )
    {
        return None;
    }
    let Expression::Identifier(read) = guard.left.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(write) =
        &single_assignment(&statement.consequent)?.left
    else {
        return None;
    };
    (read.name == write.name).then_some(write.name.as_str())
}

fn check_fallback_order(
    kind: AstKind<'_>,
    file: &str,
    source: &str,
    severity: &str,
    report: &mut Report,
) {
    let statements = match kind {
        AstKind::IfStatement(statement) => {
            if let Some(assignment) = statement.alternate.as_ref().and_then(single_assignment)
                && matches!(
                    assignment.right.without_parentheses(),
                    Expression::ConditionalExpression(_)
                )
            {
                report.finding(
                    file,
                    source,
                    statement.span.start as usize,
                    severity,
                    FALLBACK_ORDER,
                    "Make the fallback decision explicit instead of assigning a ternary in the else branch.",
                );
            }
            return;
        }
        AstKind::Program(program) => program.body.as_slice(),
        AstKind::FunctionBody(body) => body.statements.as_slice(),
        AstKind::BlockStatement(block) => block.body.as_slice(),
        AstKind::SwitchCase(case) => case.consequent.as_slice(),
        _ => return,
    };
    for pair in statements.windows(2) {
        let Some(target) = null_guard_target(&pair[1]) else {
            continue;
        };
        if null_guard_target(&pair[0]) == Some(target) {
            report.finding(
                file,
                source,
                pair[1].span().start as usize,
                severity,
                FALLBACK_ORDER,
                format!("Choose the fallback for {target} in priority order instead of repeatedly checking and assigning it."),
            );
        }
    }
}

fn check_single_use_helper(
    node: &AstNode<'_>,
    semantic: &Semantic<'_>,
    file: &str,
    source: &str,
    severity: &str,
    report: &mut Report,
) {
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
                return;
            }
            (declaration.id.get_binding_identifier(), parent_id)
        }
        AstKind::Function(function)
            if function.is_declaration() && function.body.is_some() && !function.generator =>
        {
            (function.id.as_ref(), node.id())
        }
        _ => return,
    };
    if matches!(
        nodes.parent_kind(declaration_id),
        AstKind::ExportDeclaration(_) | AstKind::ExportDefaultDeclaration(_)
    ) {
        return;
    }
    let Some(binding) = binding else { return };
    let Some(symbol) = binding.symbol_id.get() else {
        return;
    };
    let scope = semantic.scoping();
    if !scope.symbol_redeclarations(symbol).is_empty() {
        return;
    }
    let mut references = scope.get_resolved_references(symbol);
    let Some(reference) = references.next() else {
        return;
    };
    if references.next().is_some() {
        return;
    }
    let reference_id = reference.node_id();
    let reference_span = nodes.kind(reference_id).span();
    let span = node.span();
    if !matches!(nodes.parent_kind(reference_id), AstKind::CallExpression(call)
        if !call.optional && call.callee.span() == reference_span)
        || (span.start <= reference_span.start && reference_span.end <= span.end)
    {
        return;
    }
    report.finding(
        file,
        source,
        binding.span.start as usize,
        severity,
        SINGLE_USE_HELPER,
        format!(
            "{} has one direct call site. Inline it into the caller or document a local exception.",
            binding.name
        ),
    );
}

pub fn check(
    file: &str,
    source: &str,
    typescript: bool,
    inspect: bool,
    policy: &Policy<'_>,
    collect_loop_inputs: bool,
    report: &mut Report,
) -> Option<LoopInputs> {
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
    let mut loop_inputs = LoopInputs::default();
    if !inspect {
        return Some(loop_inputs);
    }
    let semantic = built.semantic;
    super::ui::check(&semantic, file, source, policy, report);
    let scope = semantic.scoping();
    // Decorator metadata may require a runtime value for a type annotation.
    let has_decorators = semantic
        .nodes()
        .iter()
        .any(|node| matches!(node.kind(), AstKind::Decorator(_)));
    let single_use_severity = policy.severity(SINGLE_USE_HELPER);
    let object_spread_severity = policy.severity(OBJECT_SPREAD_NEWLINE);
    let long_condition_severity = policy.severity(LONG_CONDITION);
    let fallback_order_severity = policy.severity(FALLBACK_ORDER);
    let collect_bindings = collect_loop_inputs && policy.severity(SIMILAR_LOOPS).is_some();
    for node in semantic.nodes().iter() {
        if collect_loop_inputs
            && let Some(specifier) = runtime_import(node.kind())
            && (specifier.starts_with("./") || specifier.starts_with("../"))
        {
            loop_inputs.imports.push(specifier.to_owned());
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
                loop_inputs.bindings.insert(
                    node.kind().span().start as usize,
                    scope.symbol_span(symbol).start as usize,
                );
            }
        }
        if let Some(severity) = single_use_severity {
            check_single_use_helper(node, &semantic, file, source, severity, report);
        }
        if let Some(severity) = fallback_order_severity {
            check_fallback_order(node.kind(), file, source, severity, report);
        }
        match node.kind() {
            AstKind::IfStatement(statement) => {
                let Some(severity) = long_condition_severity else {
                    continue;
                };
                let terms = condition_terms(&statement.test);
                if terms >= 4 {
                    report.finding(
                        file,
                        source,
                        statement.span.start as usize,
                        severity,
                        LONG_CONDITION,
                        format!("Split this if condition into smaller decisions. It combines {terms} terms."),
                    );
                }
            }
            AstKind::ObjectExpression(object) if object.properties.len() >= 4 => {
                let Some(severity) = object_spread_severity else {
                    continue;
                };
                let spreads = object
                    .properties
                    .iter()
                    .filter(|property| matches!(property, ObjectPropertyKind::SpreadProperty(_)))
                    .count();
                if spreads == 0 || object.properties.len() - spreads < 3 {
                    continue;
                }
                let shares_line = object.properties.windows(2).any(|pair| {
                    let previous_end = pair[0].span().end as usize;
                    let next_start = pair[1].span().start as usize;
                    !source[previous_end..next_start].contains(['\r', '\n', '\u{2028}', '\u{2029}'])
                });
                if shares_line {
                    report.finding(
                        file,
                        source,
                        object.span.start as usize,
                        severity,
                        OBJECT_SPREAD_NEWLINE,
                        "Put each entry on its own line in an object with a spread and at least three other properties.",
                    );
                }
            }
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
    Some(loop_inputs)
}
use std::collections::BTreeMap;

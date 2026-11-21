use oxc_ast::{
    AstKind,
    ast::{
        CallExpression, Expression, JSXAttribute, JSXAttributeItem, JSXAttributeName,
        JSXAttributeValue, JSXChild, JSXElement, JSXElementName,
    },
};
use oxc_semantic::{AstNode, Semantic};
use oxc_span::{GetSpan, Span};

use crate::{
    config::{COMPLEX_MAPPED_BUTTON, LONG_JSX_CLASS_NAME, PLAYWRIGHT_STYLE_ASSERTION, Policy},
    report::Report,
};

const CLASS_LINE_LIMIT: usize = 20;

fn attribute_name<'s>(attribute: &'s JSXAttribute<'_>) -> Option<&'s str> {
    match &attribute.name {
        JSXAttributeName::Identifier(name) => Some(name.name.as_str()),
        JSXAttributeName::NamespacedName(_) => None,
    }
}

fn attribute_expression<'s, 'a>(attribute: &'s JSXAttribute<'a>) -> Option<&'s Expression<'a>> {
    match attribute.value.as_ref()? {
        JSXAttributeValue::ExpressionContainer(container) => container.expression.as_expression(),
        _ => None,
    }
}

fn class_line_count(text: &str) -> usize {
    text.split(['\r', '\n', '\u{2028}', '\u{2029}'])
        .map(|line| line.split_whitespace().count())
        .max()
        .unwrap_or(0)
}

fn quoted_classes(span: Span, source: &str) -> usize {
    class_line_count(&source[(span.start as usize + 1)..(span.end as usize - 1)])
}

fn expression_class_count(expression: &Expression<'_>, source: &str) -> usize {
    match expression.get_inner_expression() {
        Expression::StringLiteral(literal) => quoted_classes(literal.span, source),
        Expression::TemplateLiteral(template) => template
            .quasis
            .iter()
            .map(|part| class_line_count(part.value.raw.as_str()))
            .chain(
                template
                    .expressions
                    .iter()
                    .map(|part| expression_class_count(part, source)),
            )
            .max()
            .unwrap_or(0),
        Expression::ConditionalExpression(choice) => {
            expression_class_count(&choice.consequent, source)
                .max(expression_class_count(&choice.alternate, source))
        }
        Expression::LogicalExpression(choice) => {
            let right = expression_class_count(&choice.right, source);
            if choice.operator.is_and() {
                right
            } else {
                right.max(expression_class_count(&choice.left, source))
            }
        }
        Expression::CallExpression(call)
            if matches!(call.callee.get_inner_expression(), Expression::Identifier(name)
                if matches!(name.name.as_str(), "cn" | "clsx" | "classnames" | "classNames")) =>
        {
            call.arguments
                .iter()
                .filter_map(|arg| arg.as_expression())
                .map(|arg| expression_class_count(arg, source))
                .max()
                .unwrap_or(0)
        }
        _ => 0,
    }
}

fn class_name_count(attribute: &JSXAttribute<'_>, source: &str) -> usize {
    if attribute_name(attribute) != Some("className") {
        return 0;
    }
    if let Some(JSXAttributeValue::StringLiteral(literal)) = &attribute.value {
        return quoted_classes(literal.span, source);
    }
    attribute_expression(attribute).map_or(0, |value| expression_class_count(value, source))
}

fn call_property<'a>(call: &CallExpression<'a>) -> Option<&'a str> {
    call.callee.get_member_expr()?.static_property_name()
}

fn playwright_flow_test(file: &str, semantic: &Semantic<'_>) -> bool {
    let Some((stem, _)) = file.rsplit_once('.') else {
        return false;
    };
    (stem.ends_with(".spec") || stem.ends_with(".test"))
        && !stem.ends_with(".visual.spec")
        && !stem.ends_with(".visual.test")
        && !file.split(['/', '\\']).any(|part| part == "visual")
        && semantic.nodes().iter().any(|node| {
            matches!(node.kind(), AstKind::ImportDeclaration(import)
            if import.source.value == "@playwright/test")
        })
}

fn is_expectation(kind: AstKind<'_>) -> bool {
    let AstKind::CallExpression(call) = kind else {
        return false;
    };
    if matches!(call.callee.get_inner_expression(), Expression::Identifier(name) if name.name == "expect")
    {
        return true;
    }
    call.callee.get_member_expr().is_some_and(|member| {
        member.static_property_name() == Some("poll")
            && matches!(member.object().get_inner_expression(), Expression::Identifier(name) if name.name == "expect")
    })
}

fn measurement_is_asserted(node: &AstNode<'_>, semantic: &Semantic<'_>) -> bool {
    let nodes = semantic.nodes();
    let parents = nodes.ancestor_kinds(node.id());
    if parents.clone().any(is_expectation) {
        return true;
    }
    let symbol = parents
        .take_while(|parent| {
            !matches!(
                parent,
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
        })
        .find_map(|parent| match parent {
            AstKind::VariableDeclarator(declaration) => {
                declaration.id.get_binding_identifier()?.symbol_id.get()
            }
            _ => None,
        });
    symbol.is_some_and(|symbol| {
        semantic
            .scoping()
            .get_resolved_references(symbol)
            .any(|reference| {
                nodes
                    .ancestor_kinds(reference.node_id())
                    .any(is_expectation)
            })
    })
}

fn style_assertion(node: &AstNode<'_>, semantic: &Semantic<'_>) -> bool {
    match node.kind() {
        AstKind::CallExpression(call) => match call_property(call) {
            Some("toHaveCSS") => true,
            Some("boundingBox" | "getComputedStyle") => measurement_is_asserted(node, semantic),
            None if matches!(call.callee.get_inner_expression(), Expression::Identifier(name) if name.name == "getComputedStyle") => {
                measurement_is_asserted(node, semantic)
            }
            _ => false,
        },
        AstKind::StaticMemberExpression(member)
            if matches!(
                member.property.name.as_str(),
                "scrollWidth" | "scrollHeight"
            ) =>
        {
            measurement_is_asserted(node, semantic)
        }
        _ => false,
    }
}

fn in_map_callback(node: &AstNode<'_>, semantic: &Semantic<'_>) -> bool {
    let mut parents = semantic.nodes().ancestors(node.id());
    let Some(callback) = parents.find(|parent| {
        matches!(
            parent.kind(),
            AstKind::ArrowFunctionExpression(_) | AstKind::Function(_)
        )
    }) else {
        return false;
    };
    let Some(AstKind::CallExpression(call)) = parents.map(AstNode::kind).find(|kind| {
        !matches!(
            kind,
            AstKind::ParenthesizedExpression(_) | AstKind::TSAsExpression(_)
        )
    }) else {
        return false;
    };
    call_property(call) == Some("map")
        && call
            .arguments
            .first()
            .and_then(|arg| arg.as_expression())
            .is_some_and(|arg| arg.get_inner_expression().span() == callback.kind().span())
}

fn complex_button(element: &JSXElement<'_>) -> bool {
    if !matches!(&element.opening_element.name, JSXElementName::Identifier(tag) if tag.name == "button")
    {
        return false;
    }
    let mut has_click = false;
    let mut has_dynamic_class = false;
    let mut has_state = false;
    for entry in &element.opening_element.attributes {
        let JSXAttributeItem::Attribute(attribute) = entry else {
            continue;
        };
        match attribute_name(attribute) {
            Some("onClick") => has_click = attribute_expression(attribute).is_some(),
            Some("className") => {
                has_dynamic_class = match attribute_expression(attribute)
                    .map(Expression::get_inner_expression)
                {
                    Some(Expression::TemplateLiteral(template)) => !template.expressions.is_empty(),
                    Some(Expression::ConditionalExpression(_) | Expression::CallExpression(_)) => {
                        true
                    }
                    _ => false,
                };
            }
            Some(
                "aria-pressed" | "aria-current" | "aria-selected" | "aria-checked"
                | "aria-expanded",
            ) => {
                has_state |= attribute_expression(attribute).is_some();
            }
            _ => {}
        }
    }
    has_click
        && has_dynamic_class
        && (has_state
            || element
                .children
                .iter()
                .any(|child| matches!(child, JSXChild::Element(_) | JSXChild::Fragment(_))))
}

pub(super) fn check(
    semantic: &Semantic<'_>,
    file: &str,
    source: &str,
    policy: &Policy<'_>,
    report: &mut Report,
) {
    let class_name = policy.severity(LONG_JSX_CLASS_NAME);
    let styles = policy
        .severity(PLAYWRIGHT_STYLE_ASSERTION)
        .filter(|_| playwright_flow_test(file, semantic));
    let mapped_button = policy.severity(COMPLEX_MAPPED_BUTTON);
    if class_name.is_none() && styles.is_none() && mapped_button.is_none() {
        return;
    }
    for node in semantic.nodes().iter() {
        if let Some(severity) = class_name
            && let AstKind::JSXAttribute(attribute) = node.kind()
            && class_name_count(attribute, source) >= CLASS_LINE_LIMIT
        {
            report.finding(file, source, attribute.span.start as usize, severity, LONG_JSX_CLASS_NAME,
                "Group this className across lines. One static fragment has at least 20 classes on the same line.");
        }
        if let Some(severity) = styles
            && style_assertion(node, semantic)
        {
            report.finding(file, source, node.kind().span().start as usize, severity, PLAYWRIGHT_STYLE_ASSERTION,
                "Assert the user-visible outcome in this Playwright test. Keep styling and geometry checks in a visual spec or justify the assertion.");
        }
        if let Some(severity) = mapped_button
            && let AstKind::JSXElement(element) = node.kind()
            && complex_button(element)
            && in_map_callback(node, semantic)
        {
            report.finding(file, source, element.span.start as usize, severity, COMPLEX_MAPPED_BUTTON,
                "Extract a button component with explicit props so the map callback shows the action and state.");
        }
    }
}

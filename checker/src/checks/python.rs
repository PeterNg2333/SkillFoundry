//! Python adapter: grammar roles for the shared rules, syntax validation and binding facts.

mod scope;

use std::ops::Range;

use super::{
    SourceFile,
    ast::SyntaxNode,
    language::{
        Decision, Facts, Import, Logic, Otherwise, Syntax, Wording, named_entries, standalone,
        unparenthesized,
    },
};
use crate::{
    catalog::{SIMILAR_LOOPS, SINGLE_USE_HELPER},
    config::Policy,
    report::Report,
};

pub struct Python;

pub static PYTHON: Python = Python;

static WORDING: Wording = Wording {
    cross_directory_import: "Use an absolute import instead of a relative import that leaves this directory.",
    spread_newline: "Put each entry on its own line in a dict with a ** unpacking and at least three other entries.",
};

fn is_statement(node: &SyntaxNode<'_>) -> bool {
    let kind = node.kind();
    kind.ends_with("_statement") || kind.ends_with("_definition")
}

/// The import path for `from <dots><module> import ...`, such as `.`, `./orders` or `../db/client`.
fn relative_path(node: &SyntaxNode<'_>) -> String {
    let dots = node
        .named_children()
        .find(|child| child.kind() == "import_prefix")
        .map_or(1, |prefix| prefix.text().matches('.').count());
    let mut path = if dots <= 1 {
        ".".to_owned()
    } else {
        vec![".."; dots - 1].join("/")
    };
    if let Some(module) = node
        .named_children()
        .find(|child| child.kind() == "dotted_name")
    {
        path.push('/');
        path.push_str(&module.text().replace('.', "/"));
    }
    path
}

fn imported_name(node: &SyntaxNode<'_>) -> Option<String> {
    let name = if node.kind() == "aliased_import" {
        node.field("name")?
    } else {
        node.clone()
    };
    (name.kind() == "dotted_name").then(|| name.text().replace('.', "/"))
}

fn join(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_owned()
    } else {
        format!("{base}/{name}")
    }
}

impl Syntax for Python {
    fn wording(&self) -> &'static Wording {
        &WORDING
    }

    fn comment_body<'s>(&self, comment: &'s str) -> Option<(usize, &'s str)> {
        comment.strip_prefix('#').map(|body| (1, body))
    }

    fn bypass_target(&self, comment: &SyntaxNode<'_>, source: &str) -> Option<Range<usize>> {
        if !standalone(comment, source) {
            return None;
        }
        let parent = comment.parent()?;
        let next = comment
            .next_all()
            .find(|next| next.is_named() && next.kind() != "comment")?;
        let target = if matches!(parent.kind().as_ref(), "module" | "block") {
            next
        } else if next.kind() == "block" {
            // A comment between a compound statement's header and its body precedes the
            // body's first statement.
            named_entries(&next).into_iter().next()?
        } else {
            return None;
        };
        is_statement(&target).then(|| target.range())
    }

    fn statements<'t>(&self, node: &SyntaxNode<'t>) -> Option<Vec<SyntaxNode<'t>>> {
        matches!(node.kind().as_ref(), "module" | "block").then(|| named_entries(node))
    }

    fn is_function(&self, node: &SyntaxNode<'_>) -> bool {
        node.is_named() && matches!(node.kind().as_ref(), "function_definition" | "lambda")
    }

    fn is_nesting(&self, node: &SyntaxNode<'_>) -> bool {
        matches!(
            node.kind().as_ref(),
            "if_statement"
                | "for_statement"
                | "while_statement"
                | "try_statement"
                | "with_statement"
                | "match_statement"
        )
    }

    fn is_branch(&self, node: &SyntaxNode<'_>) -> bool {
        match node.kind().as_ref() {
            "if_statement"
            | "elif_clause"
            | "for_statement"
            | "while_statement"
            | "except_clause"
            | "except_group_clause"
            | "conditional_expression"
            | "for_in_clause"
            | "if_clause"
            | "boolean_operator" => true,
            // `case _:` without a guard is the default case.
            "case_clause" => {
                let patterns: Vec<_> = node
                    .named_children()
                    .filter(|child| child.kind() == "case_pattern")
                    .collect();
                !(matches!(patterns.as_slice(), [pattern] if pattern.text() == "_")
                    && node.field("guard").is_none())
            }
            _ => false,
        }
    }

    fn decision<'t>(&self, node: &SyntaxNode<'t>) -> Option<Decision<'t>> {
        let following = match node.kind().as_ref() {
            "if_statement" => node.field_children("alternative").next(),
            "elif_clause" => node
                .next_all()
                .find(|sibling| matches!(sibling.kind().as_ref(), "elif_clause" | "else_clause")),
            _ => return None,
        };
        let otherwise = match following {
            None => Otherwise::Nothing,
            Some(clause) if clause.kind() == "elif_clause" => Otherwise::ElseIf,
            Some(clause) => Otherwise::Else(clause.field("body")?),
        };
        Some(Decision {
            offset: node.range().start,
            condition: unparenthesized(node.field("condition")?),
            body: node.field("consequence")?,
            otherwise,
        })
    }

    fn logical<'t>(
        &self,
        node: &SyntaxNode<'t>,
    ) -> Option<(Logic, SyntaxNode<'t>, SyntaxNode<'t>)> {
        if node.kind() != "boolean_operator" {
            return None;
        }
        let logic = match node.field("operator")?.text().as_ref() {
            "and" => Logic::And,
            "or" => Logic::Or,
            _ => return None,
        };
        Some((logic, node.field("left")?, node.field("right")?))
    }

    fn negated<'t>(&self, node: &SyntaxNode<'t>) -> Option<SyntaxNode<'t>> {
        (node.kind() == "not_operator")
            .then(|| node.field("argument"))
            .flatten()
    }

    fn null_test<'t>(&self, node: &SyntaxNode<'t>) -> Option<SyntaxNode<'t>> {
        if node.kind() != "comparison_operator" {
            return None;
        }
        let operators: Vec<_> = node.field_children("operators").collect();
        let operands = named_entries(node);
        let ([operator], [left, right]) = (operators.as_slice(), operands.as_slice()) else {
            return None;
        };
        if operator.text() != "is" || unparenthesized(right.clone()).kind() != "none" {
            return None;
        }
        Some(unparenthesized(left.clone())).filter(|left| left.kind() == "identifier")
    }

    fn assignment<'t>(
        &self,
        statement: &SyntaxNode<'t>,
    ) -> Option<(SyntaxNode<'t>, SyntaxNode<'t>)> {
        let statement = if statement.kind() == "block" {
            let entries = named_entries(statement);
            let [only] = entries.as_slice() else {
                return None;
            };
            only.clone()
        } else {
            statement.clone()
        };
        if statement.kind() != "expression_statement" {
            return None;
        }
        let expressions = named_entries(&statement);
        let [expression] = expressions.as_slice() else {
            return None;
        };
        if expression.kind() != "assignment" {
            return None;
        }
        Some((expression.field("left")?, expression.field("right")?))
    }

    fn is_conditional_expression(&self, node: &SyntaxNode<'_>) -> bool {
        node.kind() == "conditional_expression"
    }

    fn literal_entries<'t>(&self, node: &SyntaxNode<'t>) -> Option<(Vec<SyntaxNode<'t>>, usize)> {
        if node.kind() != "dictionary" {
            return None;
        }
        let entries = named_entries(node);
        let spreads = entries
            .iter()
            .filter(|entry| entry.kind() == "dictionary_splat")
            .count();
        Some((entries, spreads))
    }

    fn imports(&self, root: &SyntaxNode<'_>) -> Vec<Import> {
        let mut imports = Vec::new();
        for node in root.dfs() {
            match node.kind().as_ref() {
                "import_statement" => {
                    for name in node.field_children("name") {
                        if let Some(path) = imported_name(&name) {
                            imports.push(Import {
                                offset: name.range().start,
                                path,
                                relative: false,
                                members: Vec::new(),
                            });
                        }
                    }
                }
                "import_from_statement" => {
                    let Some(module) = node.field("module_name") else {
                        continue;
                    };
                    let members = node
                        .field_children("name")
                        .filter_map(|name| imported_name(&name))
                        .collect();
                    let relative = module.kind() == "relative_import";
                    imports.push(Import {
                        offset: module.range().start,
                        path: if relative {
                            relative_path(&module)
                        } else {
                            module.text().replace('.', "/")
                        },
                        relative,
                        members,
                    });
                }
                _ => {}
            }
        }
        imports
    }

    fn module_files(&self, base: &str, members: &[String]) -> Vec<String> {
        let mut files = Vec::new();
        for member in members {
            files.push(format!("{}.py", join(base, member)));
            files.push(join(&join(base, member), "__init__.py"));
        }
        if !base.is_empty() {
            files.push(format!("{base}.py"));
            files.push(format!("{base}.pyi"));
        }
        files.push(join(base, "__init__.py"));
        files.push(join(base, "__init__.pyi"));
        files
    }

    fn loop_body<'t>(&self, node: &SyntaxNode<'t>) -> Option<SyntaxNode<'t>> {
        matches!(node.kind().as_ref(), "for_statement" | "while_statement")
            .then(|| node.field("body"))
            .flatten()
    }

    fn skips_token(&self, kind: &str) -> bool {
        matches!(kind, "comment" | ";" | "type")
    }
}

fn syntax_error(root: &SyntaxNode<'_>) -> Option<(usize, &'static str)> {
    root.dfs().find_map(|node| {
        if node.is_error() || node.is_missing() {
            Some((node.range().start, "Invalid Python syntax."))
        } else if node.is_named()
            && matches!(node.kind().as_ref(), "print_statement" | "exec_statement")
        {
            // The grammar accepts these Python 2 statements; Python 3 rejects them.
            Some((
                node.range().start,
                "Python 2 print and exec statements are invalid in Python 3; call print() or exec().",
            ))
        } else {
            None
        }
    })
}

/// Validates syntax and, when a rule needs them, resolves bindings and module references.
/// Returns None after reporting a syntax error.
pub fn facts(
    root: &SyntaxNode<'_>,
    input: &SourceFile<'_>,
    inspect: bool,
    policy: &Policy<'_>,
    collect_loop_inputs: bool,
    report: &mut Report,
) -> Option<Facts> {
    if let Some((offset, message)) = syntax_error(root) {
        report.finding(input.path, input.text, offset, "error", "syntax", message);
        return None;
    }
    let mut facts = Facts::default();
    if !inspect {
        return Some(facts);
    }
    let collect_helpers = policy.severity(SINGLE_USE_HELPER).is_some();
    let collect_bindings = collect_loop_inputs && policy.severity(SIMILAR_LOOPS).is_some();
    if collect_helpers || collect_bindings {
        let analysis = scope::analyze(root);
        facts.helpers = analysis.helpers;
        if collect_bindings {
            facts.bindings = analysis.bindings;
        }
    }
    if collect_loop_inputs {
        facts.imports = PYTHON.imports(root);
    }
    Some(facts)
}

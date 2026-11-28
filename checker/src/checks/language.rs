//! The contract between shared native rules and language adapters.
//!
//! Shared rules (`metrics`, `decisions`, `literals`, `imports`, `helpers`, `directives` and
//! `loop_similarity`) ask a [`Syntax`] for syntactic roles instead of naming grammar node kinds.
//! An adapter maps its tree-sitter grammar onto these roles, and a binding analysis supplies
//! [`Facts`]. A rule written against this contract runs for every language whose adapter
//! implements it and whose entry in `crate::catalog` lists the rule.

use std::{collections::BTreeMap, ops::Range};

use ast_grep_language::SupportLang;

use super::{ast::SyntaxNode, helpers::Helper, javascript, python};
use crate::catalog;

/// A source language analyzed by the shared rules, with its grammar variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    JavaScript,
    TypeScript,
    Tsx,
    Python,
}

impl Language {
    pub fn of(kind: &str, path: &str) -> Option<Self> {
        match kind {
            catalog::JAVASCRIPT => Some(Self::JavaScript),
            catalog::TYPESCRIPT if path.to_lowercase().ends_with(".tsx") => Some(Self::Tsx),
            catalog::TYPESCRIPT => Some(Self::TypeScript),
            catalog::PYTHON => Some(Self::Python),
            _ => None,
        }
    }

    /// The catalog language, which is also the selected file kind and check ID.
    pub fn kind(self) -> &'static str {
        match self {
            Self::JavaScript => catalog::JAVASCRIPT,
            Self::TypeScript | Self::Tsx => catalog::TYPESCRIPT,
            Self::Python => catalog::PYTHON,
        }
    }

    pub fn grammar(self) -> SupportLang {
        match self {
            Self::JavaScript => SupportLang::JavaScript,
            Self::TypeScript => SupportLang::TypeScript,
            Self::Tsx => SupportLang::Tsx,
            Self::Python => SupportLang::Python,
        }
    }

    pub fn syntax(self) -> &'static dyn Syntax {
        match self {
            Self::Python => &python::PYTHON,
            _ => &javascript::ECMASCRIPT,
        }
    }

    /// Files compared by cross-file rules such as similar loops.
    pub fn family(self) -> &'static str {
        match self {
            Self::Python => catalog::PYTHON,
            _ => "ecmascript",
        }
    }
}

/// An `if` or `elif` decision.
pub struct Decision<'t> {
    pub offset: usize,
    pub condition: SyntaxNode<'t>,
    pub body: SyntaxNode<'t>,
    pub otherwise: Otherwise<'t>,
}

/// What a decision runs when its condition is false.
pub enum Otherwise<'t> {
    Nothing,
    ElseIf,
    Else(SyntaxNode<'t>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Logic {
    And,
    Or,
}

/// A module reference. Relative paths use slash form, such as `.`, `./orders` or `../db/client`.
/// Absolute paths are package names in slash form, such as `react` or `app/services`.
pub struct Import {
    pub offset: usize,
    pub path: String,
    pub relative: bool,
    /// Imported names that may be modules themselves, as in Python's `from . import orders`.
    pub members: Vec<String>,
}

/// Diagnostic wording that names a language's own constructs.
pub struct Wording {
    pub cross_directory_import: &'static str,
    pub spread_newline: &'static str,
}

/// Results of a language's binding analysis.
#[derive(Default)]
pub struct Facts {
    /// Private function bindings with every resolved reference.
    pub helpers: Vec<Helper>,
    /// Identifier offset to the offset of the binding it resolves to.
    pub bindings: BTreeMap<usize, usize>,
    /// Runtime module references used for import-path evidence.
    pub imports: Vec<Import>,
}

/// Syntactic roles a language adapter supplies to the shared rules.
pub trait Syntax: Sync {
    fn wording(&self) -> &'static Wording;

    /// The comment text without its markers, with the marker length before it.
    fn comment_body<'s>(&self, comment: &'s str) -> Option<(usize, &'s str)>;
    /// The statement or declaration a standalone `@bypass` comment applies to.
    fn bypass_target(&self, comment: &SyntaxNode<'_>, source: &str) -> Option<Range<usize>>;
    /// The statements of a statement list such as a module, block or case body.
    fn statements<'t>(&self, node: &SyntaxNode<'t>) -> Option<Vec<SyntaxNode<'t>>>;

    fn is_function(&self, node: &SyntaxNode<'_>) -> bool;
    /// A control structure that adds one nesting level for `max-depth`.
    fn is_nesting(&self, node: &SyntaxNode<'_>) -> bool;
    /// A decision point that adds one to `complexity`.
    fn is_branch(&self, node: &SyntaxNode<'_>) -> bool;

    fn decision<'t>(&self, node: &SyntaxNode<'t>) -> Option<Decision<'t>>;
    /// A short-circuit `and` or `or`. Nullish coalescing is not a logical decision term.
    fn logical<'t>(&self, node: &SyntaxNode<'t>)
    -> Option<(Logic, SyntaxNode<'t>, SyntaxNode<'t>)>;
    fn negated<'t>(&self, node: &SyntaxNode<'t>) -> Option<SyntaxNode<'t>>;
    /// The identifier tested by a strict null comparison such as `x === null` or `x is None`.
    fn null_test<'t>(&self, node: &SyntaxNode<'t>) -> Option<SyntaxNode<'t>>;
    /// Target and value of a statement, or single-statement block, that is one plain assignment.
    fn assignment<'t>(
        &self,
        statement: &SyntaxNode<'t>,
    ) -> Option<(SyntaxNode<'t>, SyntaxNode<'t>)>;
    fn is_conditional_expression(&self, node: &SyntaxNode<'_>) -> bool;

    /// Entries of an object or dictionary literal and how many of them are spreads.
    fn literal_entries<'t>(&self, node: &SyntaxNode<'t>) -> Option<(Vec<SyntaxNode<'t>>, usize)>;

    fn imports(&self, root: &SyntaxNode<'_>) -> Vec<Import>;
    /// Candidate project files for a module path, in preference order.
    fn module_files(&self, base: &str, members: &[String]) -> Vec<String>;

    fn loop_body<'t>(&self, node: &SyntaxNode<'t>) -> Option<SyntaxNode<'t>>;
    /// Tokens left out of loop-body fingerprints, such as separators and type annotations.
    fn skips_token(&self, kind: &str) -> bool;
}

/// The expression inside any number of parentheses.
pub fn unparenthesized(mut node: SyntaxNode<'_>) -> SyntaxNode<'_> {
    while node.kind() == "parenthesized_expression" {
        let Some(inner) = node
            .named_children()
            .find(|child| child.kind() != "comment")
        else {
            break;
        };
        node = inner;
    }
    node
}

/// Named children other than comments.
pub fn named_entries<'t>(node: &SyntaxNode<'t>) -> Vec<SyntaxNode<'t>> {
    node.named_children()
        .filter(|child| child.kind() != "comment")
        .collect()
}

/// Whether a subtree parsed without error recovery. A grammar can recover from input that the
/// language's own parser accepts, such as a TSX generic call, so rules skip damaged subtrees.
pub fn intact(node: &SyntaxNode<'_>) -> bool {
    !node.dfs().any(|node| node.is_error() || node.is_missing())
}

/// Whether a comment starts its own line.
pub fn standalone(comment: &SyntaxNode<'_>, source: &str) -> bool {
    let start = comment.range().start;
    let line_start = source[..start].rfind('\n').map_or(0, |end| end + 1);
    source[line_start..start].trim().is_empty()
}

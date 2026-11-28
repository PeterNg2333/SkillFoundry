//! Python name binding for the shared rules.
//!
//! Every assignment target, definition, parameter, import alias, `for`/`with`/`except` target,
//! walrus target and match capture binds a name in its scope. A scope's own bindings make a
//! name local anywhere in that scope, as in Python. References resolve through enclosing
//! function and module scopes; class bodies are hidden from the functions they contain, and
//! `global` and `nonlocal` redirect both bindings and references.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::Range,
};

use super::super::{
    ast::SyntaxNode,
    helpers::{Helper, Reference},
    language::named_entries,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Module,
    Function,
    Class,
    Comprehension,
}

struct Function {
    offset: usize,
    declaration: Range<usize>,
    private: bool,
}

struct Name {
    first: usize,
    sites: usize,
    function: Option<Function>,
}

struct Scope {
    kind: Kind,
    parent: Option<usize>,
    names: HashMap<String, Name>,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
}

struct Use {
    scope: usize,
    name: String,
    offset: usize,
    direct_call: bool,
}

pub struct Analysis {
    /// Private functions bound once in their scope, with resolved references.
    pub helpers: Vec<Helper>,
    /// Identifier offset to the first binding site of the name it resolves to.
    pub bindings: BTreeMap<usize, usize>,
}

struct Resolver {
    scopes: Vec<Scope>,
    uses: Vec<Use>,
    sites: Vec<(usize, String, usize)>,
    exported: HashSet<String>,
}

/// Names listed in a module-level `__all__`, which are public even with a leading underscore.
fn exported(root: &SyntaxNode<'_>) -> HashSet<String> {
    let mut names = HashSet::new();
    for statement in named_entries(root) {
        if statement.kind() != "expression_statement" {
            continue;
        }
        for expression in named_entries(&statement) {
            if matches!(
                expression.kind().as_ref(),
                "assignment" | "augmented_assignment"
            ) && expression
                .field("left")
                .is_some_and(|left| left.text() == "__all__")
                && let Some(value) = expression.field("right")
            {
                names.extend(
                    value
                        .dfs()
                        .filter(|node| node.kind() == "string_content")
                        .map(|node| node.text().into_owned()),
                );
            }
        }
    }
    names
}

/// Whether a function body yields, excluding nested functions and classes.
fn yields(node: &SyntaxNode<'_>) -> bool {
    node.named_children()
        .any(|child| match child.kind().as_ref() {
            "function_definition" | "lambda" | "class_definition" => false,
            "yield" => true,
            _ => yields(&child),
        })
}

fn same(left: Option<&SyntaxNode<'_>>, right: &SyntaxNode<'_>) -> bool {
    left.is_some_and(|left| left.node_id() == right.node_id())
}

impl Resolver {
    fn push(&mut self, kind: Kind, parent: usize) -> usize {
        self.scopes.push(Scope {
            kind,
            parent: Some(parent),
            names: HashMap::new(),
            globals: HashSet::new(),
            nonlocals: HashSet::new(),
        });
        self.scopes.len() - 1
    }

    /// The nearest enclosing scope that binds `name`, skipping class bodies.
    fn enclosing(&self, mut parent: Option<usize>, name: &str) -> Option<usize> {
        while let Some(id) = parent {
            let scope = &self.scopes[id];
            if scope.kind != Kind::Class {
                if scope.globals.contains(name) {
                    return self.scopes[0].names.contains_key(name).then_some(0);
                }
                if scope.names.contains_key(name) {
                    return Some(id);
                }
            }
            parent = scope.parent;
        }
        None
    }

    fn resolve(&self, id: usize, name: &str) -> Option<usize> {
        let scope = &self.scopes[id];
        if scope.globals.contains(name) {
            return self.scopes[0].names.contains_key(name).then_some(0);
        }
        if scope.nonlocals.contains(name) {
            return self.enclosing(scope.parent, name);
        }
        if scope.names.contains_key(name) {
            return Some(id);
        }
        self.enclosing(scope.parent, name)
    }

    fn bind(&mut self, id: usize, name: &str, offset: usize, function: Option<Function>) {
        let scope = &self.scopes[id];
        let id = if scope.globals.contains(name) {
            0
        } else if scope.nonlocals.contains(name) {
            self.enclosing(scope.parent, name).unwrap_or(id)
        } else {
            id
        };
        let entry = self.scopes[id]
            .names
            .entry(name.to_owned())
            .or_insert(Name {
                first: offset,
                sites: 0,
                function: None,
            });
        entry.sites += 1;
        if function.is_some() {
            entry.function = function;
        }
        self.sites.push((id, name.to_owned(), offset));
    }

    fn reference(&mut self, node: &SyntaxNode<'_>, scope: usize) {
        let direct_call = node.parent().is_some_and(|parent| {
            parent.kind() == "call" && same(parent.field("function").as_ref(), node)
        });
        self.uses.push(Use {
            scope,
            name: node.text().into_owned(),
            offset: node.range().start,
            direct_call,
        });
    }

    fn visit_children(&mut self, node: &SyntaxNode<'_>, scope: usize) {
        for child in node.named_children() {
            self.visit(&child, scope);
        }
    }

    fn visit(&mut self, node: &SyntaxNode<'_>, scope: usize) {
        match node.kind().as_ref() {
            "identifier" => self.reference(node, scope),
            "comment" => {}
            "decorated_definition" => {
                for decorator in node
                    .named_children()
                    .filter(|child| child.kind() == "decorator")
                {
                    self.visit(&decorator, scope);
                }
                if let Some(definition) = node.field("definition") {
                    self.definition(&definition, scope, true);
                }
            }
            "function_definition" | "class_definition" => self.definition(node, scope, false),
            "lambda" => {
                let inner = self.push(Kind::Function, scope);
                if let Some(parameters) = node.field("parameters") {
                    self.parameters(&parameters, scope, inner);
                }
                if let Some(body) = node.field("body") {
                    self.visit(&body, inner);
                }
            }
            "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "generator_expression" => self.comprehension(node, scope),
            "assignment" | "for_statement" | "for_in_clause" => {
                let target = node.field("left");
                for child in node.named_children() {
                    if same(target.as_ref(), &child) {
                        self.targets(&child, scope);
                    } else {
                        self.visit(&child, scope);
                    }
                }
            }
            "augmented_assignment" => {
                if let Some(target) = node.field("left") {
                    if target.kind() == "identifier" {
                        self.bind(scope, &target.text(), target.range().start, None);
                    }
                    self.visit(&target, scope);
                }
                if let Some(value) = node.field("right") {
                    self.visit(&value, scope);
                }
            }
            "named_expression" => {
                if let Some(value) = node.field("value") {
                    self.visit(&value, scope);
                }
                let mut target = scope;
                while self.scopes[target].kind == Kind::Comprehension {
                    target = self.scopes[target].parent.unwrap_or(0);
                }
                if let Some(name) = node.field("name") {
                    self.bind(target, &name.text(), name.range().start, None);
                }
            }
            "as_pattern" => {
                let alias = node.field("alias");
                for child in node.named_children() {
                    if same(alias.as_ref(), &child) {
                        self.targets(&child, scope);
                    } else {
                        self.visit(&child, scope);
                    }
                }
            }
            "import_statement" | "import_from_statement" => {
                for name in node.field_children("name") {
                    let bound = if name.kind() == "aliased_import" {
                        name.field("alias")
                    } else {
                        name.named_children()
                            .find(|part| part.kind() == "identifier")
                    };
                    if let Some(bound) = bound {
                        self.bind(scope, &bound.text(), bound.range().start, None);
                    }
                }
            }
            "future_import_statement" => {}
            "global_statement" | "nonlocal_statement" => {
                let global = node.kind() == "global_statement";
                for name in node
                    .named_children()
                    .filter(|child| child.kind() == "identifier")
                {
                    let names = if global {
                        &mut self.scopes[scope].globals
                    } else {
                        &mut self.scopes[scope].nonlocals
                    };
                    names.insert(name.text().into_owned());
                }
            }
            "delete_statement" => {
                for target in node.named_children() {
                    self.targets(&target, scope);
                }
            }
            "keyword_argument" => {
                if let Some(value) = node.field("value") {
                    self.visit(&value, scope);
                }
            }
            "attribute" => {
                if let Some(object) = node.field("object") {
                    self.visit(&object, scope);
                }
            }
            "case_pattern" => self.pattern(node, scope),
            _ => self.visit_children(node, scope),
        }
    }

    fn definition(&mut self, node: &SyntaxNode<'_>, scope: usize, decorated: bool) {
        let name = node.field("name");
        if node.kind() == "class_definition" {
            if let Some(name) = &name {
                self.bind(scope, &name.text(), name.range().start, None);
            }
            for field in ["type_parameters", "superclasses"] {
                if let Some(child) = node.field(field) {
                    self.visit(&child, scope);
                }
            }
            let inner = self.push(Kind::Class, scope);
            if let Some(body) = node.field("body") {
                self.visit(&body, inner);
            }
            return;
        }
        let inner = self.push(Kind::Function, scope);
        if let Some(parameters) = node.field("parameters") {
            self.parameters(&parameters, scope, inner);
        }
        if let Some(returns) = node.field("return_type") {
            self.visit(&returns, scope);
        }
        if let Some(name) = &name {
            let text = name.text();
            let dunder = text.starts_with("__") && text.ends_with("__");
            let private = match self.scopes[scope].kind {
                Kind::Function => true,
                Kind::Module => {
                    text.starts_with('_') && !dunder && !self.exported.contains(text.as_ref())
                }
                Kind::Class | Kind::Comprehension => false,
            };
            let generator = node.field("body").is_some_and(|body| yields(&body));
            let function = Function {
                offset: name.range().start,
                declaration: node.range(),
                private: private && !decorated && !dunder && !generator,
            };
            self.bind(scope, &text, name.range().start, Some(function));
        }
        if let Some(body) = node.field("body") {
            self.visit(&body, inner);
        }
    }

    /// Defaults and annotations evaluate in the enclosing scope; names bind in the function.
    fn parameters(&mut self, parameters: &SyntaxNode<'_>, outer: usize, inner: usize) {
        for parameter in parameters.named_children() {
            match parameter.kind().as_ref() {
                "default_parameter" | "typed_default_parameter" | "typed_parameter" => {
                    let name = parameter.field("name");
                    let annotation = parameter.field("type");
                    let value = parameter.field("value");
                    for child in parameter.named_children() {
                        if same(annotation.as_ref(), &child) || same(value.as_ref(), &child) {
                            self.visit(&child, outer);
                        } else if name.is_none() || same(name.as_ref(), &child) {
                            self.targets(&child, inner);
                        }
                    }
                }
                "keyword_separator" | "positional_separator" | "comment" => {}
                _ => self.targets(&parameter, inner),
            }
        }
    }

    /// The first iterable evaluates in the enclosing scope; the rest binds in the comprehension.
    fn comprehension(&mut self, node: &SyntaxNode<'_>, scope: usize) {
        let inner = self.push(Kind::Comprehension, scope);
        let mut first = true;
        for child in node.named_children() {
            if child.kind() != "for_in_clause" {
                self.visit(&child, inner);
                continue;
            }
            if let Some(iterable) = child.field("right") {
                self.visit(&iterable, if first { scope } else { inner });
            }
            first = false;
            if let Some(target) = child.field("left") {
                self.targets(&target, inner);
            }
        }
    }

    fn targets(&mut self, node: &SyntaxNode<'_>, scope: usize) {
        match node.kind().as_ref() {
            "identifier" => self.bind(scope, &node.text(), node.range().start, None),
            "pattern_list"
            | "tuple_pattern"
            | "list_pattern"
            | "tuple"
            | "list"
            | "expression_list"
            | "parenthesized_expression"
            | "list_splat_pattern"
            | "dictionary_splat_pattern"
            | "list_splat"
            | "as_pattern_target" => {
                for child in node.named_children() {
                    self.targets(&child, scope);
                }
            }
            _ => self.visit(node, scope),
        }
    }

    /// Match patterns: a bare name captures; a dotted name or class name is a value reference.
    fn pattern(&mut self, node: &SyntaxNode<'_>, scope: usize) {
        match node.kind().as_ref() {
            "dotted_name" => {
                let parts: Vec<_> = node
                    .named_children()
                    .filter(|part| part.kind() == "identifier")
                    .collect();
                let capture = parts.len() == 1
                    && node
                        .parent()
                        .is_some_and(|parent| parent.kind() == "case_pattern");
                match parts.first() {
                    Some(part) if capture => {
                        self.bind(scope, &part.text(), part.range().start, None)
                    }
                    Some(part) => self.reference(part, scope),
                    None => {}
                }
            }
            "identifier" => self.bind(scope, &node.text(), node.range().start, None),
            "keyword_pattern" => {
                for child in node.named_children().skip(1) {
                    self.pattern(&child, scope);
                }
            }
            "as_pattern" => {
                let alias = node.field("alias");
                for child in node.named_children() {
                    if same(alias.as_ref(), &child) {
                        self.targets(&child, scope);
                    } else {
                        self.pattern(&child, scope);
                    }
                }
            }
            "if_clause" => self.visit(node, scope),
            _ => {
                for child in node.named_children() {
                    self.pattern(&child, scope);
                }
            }
        }
    }
}

pub fn analyze(root: &SyntaxNode<'_>) -> Analysis {
    let mut resolver = Resolver {
        scopes: vec![Scope {
            kind: Kind::Module,
            parent: None,
            names: HashMap::new(),
            globals: HashSet::new(),
            nonlocals: HashSet::new(),
        }],
        uses: Vec::new(),
        sites: Vec::new(),
        exported: exported(root),
    };
    resolver.visit_children(root, 0);
    let mut bindings = BTreeMap::new();
    let mut references: HashMap<(usize, &str), Vec<Reference>> = HashMap::new();
    for usage in &resolver.uses {
        let Some(scope) = resolver.resolve(usage.scope, &usage.name) else {
            continue;
        };
        bindings.insert(
            usage.offset,
            resolver.scopes[scope].names[&usage.name].first,
        );
        references
            .entry((scope, usage.name.as_str()))
            .or_default()
            .push(Reference {
                offset: usage.offset,
                direct_call: usage.direct_call,
            });
    }
    for (scope, name, offset) in &resolver.sites {
        bindings.insert(*offset, resolver.scopes[*scope].names[name].first);
    }
    let mut helpers = Vec::new();
    for (id, scope) in resolver.scopes.iter().enumerate() {
        for (name, entry) in &scope.names {
            let Some(function) = entry.function.as_ref().filter(|function| function.private) else {
                continue;
            };
            if entry.sites != 1 {
                continue;
            }
            helpers.push(Helper {
                name: name.clone(),
                offset: function.offset,
                declaration: function.declaration.clone(),
                references: references.remove(&(id, name.as_str())).unwrap_or_default(),
            });
        }
    }
    Analysis { helpers, bindings }
}

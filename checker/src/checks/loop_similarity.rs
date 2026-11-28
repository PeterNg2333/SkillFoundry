//! `maintainable/similar-loops`: cross-file fingerprints of loop bodies in one language family.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    ops::Range,
    path::Path,
    rc::Rc,
};

use super::{
    SourceFile,
    ast::SyntaxNode,
    directives::LoopBypass,
    language::{Facts, Import, Language, Syntax},
};
use crate::{
    catalog::SIMILAR_LOOPS,
    config::{LoopSimilarity, Settings, rule_severity},
    report::{Bypass, Finding, Location, Report, source_location},
    selection::normalized,
};

#[derive(Clone, Eq, Hash, PartialEq)]
enum Token {
    Local(usize),
    Text(Rc<str>),
}

type Trigrams = HashMap<[Token; 3], usize>;

struct Loop {
    offset: usize,
    line: usize,
    column: usize,
    trigrams: Trigrams,
    count: usize,
}

pub struct File {
    path: String,
    family: &'static str,
    syntax: &'static dyn Syntax,
    imports: Vec<Import>,
    severity: Option<&'static str>,
    loops: Vec<Loop>,
    bypasses: Vec<LoopBypass>,
}

pub fn requested(settings: &Settings, rules: &[String]) -> bool {
    (rules.is_empty() || rules.iter().any(|rule| rule == SIMILAR_LOOPS))
        && std::iter::once(&settings.rules)
            .chain(settings.overrides.iter().map(|entry| &entry.rules))
            .any(|rules| rules.get(SIMILAR_LOOPS).and_then(rule_severity).is_some())
}

struct Body<'a> {
    syntax: &'a dyn Syntax,
    range: Range<usize>,
    bindings: &'a BTreeMap<usize, usize>,
}

fn tokens(
    node: &SyntaxNode<'_>,
    body: &Body<'_>,
    locals: &mut BTreeMap<usize, usize>,
    output: &mut Vec<Token>,
) {
    let kind = node.kind();
    if body.syntax.skips_token(&kind) {
        return;
    }
    let children = node.children();
    if children.len() > 0 {
        for child in children {
            tokens(&child, body, locals, output);
        }
    } else if kind == "identifier"
        && let Some(declaration) = body.bindings.get(&node.range().start)
        && body.range.contains(declaration)
    {
        let next = locals.len();
        output.push(Token::Local(*locals.entry(*declaration).or_insert(next)));
    } else {
        output.push(Token::Text(Rc::from(node.text().as_ref())));
    }
}

pub fn collect(
    root: &SyntaxNode<'_>,
    input: &SourceFile<'_>,
    language: Language,
    facts: Facts,
    severity: Option<&'static str>,
    options: LoopSimilarity,
    bypasses: Vec<LoopBypass>,
) -> File {
    let syntax = language.syntax();
    let mut loops = Vec::new();
    if severity.is_some() {
        let minimum = options.min_body_tokens.expect("embedded minimum body size");
        for node in root.dfs().filter(|node| node.is_named()) {
            let Some(body) = syntax.loop_body(&node) else {
                continue;
            };
            if body.dfs().any(|node| node.is_error() || node.is_missing()) {
                continue;
            }
            let mut body_tokens = Vec::new();
            tokens(
                &body,
                &Body {
                    syntax,
                    range: node.range(),
                    bindings: &facts.bindings,
                },
                &mut BTreeMap::new(),
                &mut body_tokens,
            );
            if body_tokens.len() < minimum {
                continue;
            }
            let mut trigrams = HashMap::new();
            for window in body_tokens.windows(3) {
                let [first, second, third] = window else {
                    unreachable!()
                };
                *trigrams
                    .entry([first.clone(), second.clone(), third.clone()])
                    .or_insert(0) += 1;
            }
            let offset = node.range().start;
            let (line, column) = source_location(input.text, offset);
            loops.push(Loop {
                offset,
                line,
                column,
                trigrams,
                count: body_tokens.len() - 2,
            });
        }
    }
    File {
        path: input.path.to_owned(),
        family: language.family(),
        syntax,
        imports: facts.imports,
        severity,
        loops,
        bypasses,
    }
}

/// Selected files an import refers to.
fn resolve_import(
    root: &Path,
    file: &File,
    import: &Import,
    ids: &BTreeMap<&str, usize>,
) -> Vec<usize> {
    let base = if import.relative {
        let base = normalized(&root.join(&file.path).with_file_name("").join(&import.path));
        let Some(relative) = base.strip_prefix(root).ok().and_then(Path::to_str) else {
            return Vec::new();
        };
        relative.replace('\\', "/")
    } else {
        import.path.clone()
    };
    let find = |candidate: &String| {
        ids.get(candidate.as_str()).copied().or_else(|| {
            if import.relative {
                return None;
            }
            // An absolute module can live below a source root such as `src/`.
            let suffix = format!("/{candidate}");
            let mut matches = ids.iter().filter(|(path, _)| path.ends_with(&suffix));
            let first = matches.next().map(|(_, id)| *id);
            first.filter(|_| matches.next().is_none())
        })
    };
    let candidates = file.syntax.module_files(&base, &import.members);
    // Python's `from . import a, b` can name several modules; the member candidates come first.
    let (members, module) = candidates.split_at(2 * import.members.len());
    let targets: Vec<_> = members.iter().filter_map(find).collect();
    if !targets.is_empty() {
        return targets;
    }
    module.iter().find_map(find).into_iter().collect()
}

struct Graph {
    outgoing: Vec<BTreeSet<usize>>,
    neighbors: Vec<BTreeSet<usize>>,
}

impl Graph {
    fn new(root: &Path, files: &[File]) -> Self {
        let ids = files
            .iter()
            .enumerate()
            .map(|(index, file)| (file.path.as_str(), index))
            .collect();
        let mut graph = Self {
            outgoing: vec![BTreeSet::new(); files.len()],
            neighbors: vec![BTreeSet::new(); files.len()],
        };
        for (index, file) in files.iter().enumerate() {
            for import in &file.imports {
                for target in resolve_import(root, file, import, &ids) {
                    if target != index && files[target].family == file.family {
                        graph.outgoing[index].insert(target);
                        graph.neighbors[index].insert(target);
                        graph.neighbors[target].insert(index);
                    }
                }
            }
        }
        graph
    }

    fn path(&self, left: usize, right: usize, paths: &[String]) -> Option<String> {
        let arrow = |from: usize, to: usize| {
            if self.outgoing[from].contains(&to) {
                "->"
            } else {
                "<-"
            }
        };
        if self.neighbors[left].contains(&right) {
            return Some(format!(
                "{} {} {}",
                paths[left],
                arrow(left, right),
                paths[right]
            ));
        }
        let middle = *self.neighbors[left]
            .intersection(&self.neighbors[right])
            .min_by_key(|middle| {
                // Prefer a caller or a chain over a dependency both files happen to use.
                (
                    self.outgoing[left].contains(middle) && self.outgoing[right].contains(middle),
                    **middle,
                )
            })?;
        Some(format!(
            "{} {} {} {} {}",
            paths[left],
            arrow(left, middle),
            paths[middle],
            arrow(middle, right),
            paths[right]
        ))
    }
}

fn similarity(left: &Loop, right: &Loop, minimum: usize) -> Option<usize> {
    if left.count.min(right.count) * 100 < minimum * left.count.max(right.count) {
        return None;
    }
    let shared: usize = left
        .trigrams
        .iter()
        .map(|(gram, count)| *count.min(right.trigrams.get(gram).unwrap_or(&0)))
        .sum();
    let score = 100 * shared / (left.count + right.count - shared);
    (score >= minimum).then_some(score)
}

fn matching_bypass(bypasses: &mut [LoopBypass], offset: usize) -> Option<&LoopBypass> {
    let bypass = bypasses
        .iter_mut()
        .find(|bypass| bypass.target.contains(&offset))?;
    bypass.used = true;
    Some(bypass)
}

pub fn finish(root: &Path, mut files: Vec<File>, options: LoopSimilarity, report: &mut Report) {
    if files.is_empty() {
        return;
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let minimum = options.min_similarity.expect("embedded minimum similarity");
    let paths: Vec<_> = files.iter().map(|file| file.path.clone()).collect();
    let graph = Graph::new(root, &files);
    let candidates: Vec<_> = files
        .iter()
        .enumerate()
        .filter_map(|(index, file)| {
            file.severity
                .filter(|_| !file.loops.is_empty())
                .map(|severity| (index, severity))
        })
        .collect();
    for (position, &(left_index, left_severity)) in candidates.iter().enumerate() {
        for &(right_index, right_severity) in &candidates[position + 1..] {
            if files[left_index].family != files[right_index].family {
                continue;
            }
            let [left, right] = files
                .get_disjoint_mut([left_index, right_index])
                .expect("distinct files");
            let severity = if left_severity == "error" {
                left_severity
            } else {
                right_severity
            };
            for a in &left.loops {
                for b in &right.loops {
                    let Some(score) = similarity(a, b, minimum) else {
                        continue;
                    };
                    let left_bypass = matching_bypass(&mut left.bypasses, a.offset);
                    let right_bypass = matching_bypass(&mut right.bypasses, b.offset);
                    let (file, primary, other_file, peer) =
                        if left_bypass.is_none() && right_bypass.is_some() {
                            (&right.path, b, &left.path, a)
                        } else {
                            (&left.path, a, &right.path, b)
                        };
                    let mut message = format!(
                        "Loop body is {score}% similar to {other_file}:{}:{}.",
                        peer.line, peer.column
                    );
                    if let Some(path) = graph.path(left_index, right_index, &paths) {
                        message.push_str(&format!(" Import path: {path}."));
                    }
                    let finding = Finding {
                        file: file.clone(),
                        line: primary.line,
                        column: primary.column,
                        severity: severity.to_owned(),
                        rule: SIMILAR_LOOPS.to_owned(),
                        message,
                        origin: "static",
                        offset: primary.offset,
                        related: Some(Location {
                            file: other_file.clone(),
                            line: peer.line,
                            column: peer.column,
                        }),
                    };
                    if let Some(bypass) = left_bypass.or(right_bypass) {
                        report.bypasses.push(Bypass {
                            finding,
                            directive_line: Some(bypass.line),
                            reason: bypass.reason.clone(),
                        });
                    } else {
                        report.diagnostics.push(finding);
                    }
                }
            }
        }
    }
    for file in files {
        for bypass in file.bypasses {
            bypass.finish(&file.path, report);
        }
    }
}

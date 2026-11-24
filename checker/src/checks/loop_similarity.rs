use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    ops::Range,
    path::Path,
    rc::Rc,
};

use super::{SourceFile, ast::SyntaxNode, directives::LoopBypass, semantics::LoopInputs};
use crate::{
    config::{LoopSimilarity, SIMILAR_LOOPS, Settings, rule_severity},
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
    imports: Vec<String>,
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

fn tokens(
    node: &SyntaxNode<'_>,
    loop_range: &Range<usize>,
    bindings: &BTreeMap<usize, usize>,
    locals: &mut BTreeMap<usize, usize>,
    output: &mut Vec<Token>,
) {
    let kind = node.kind();
    if matches!(kind.as_ref(), "comment" | ";" | "type_annotation") {
        return;
    }
    let children = node.children();
    if children.len() > 0 {
        for child in children {
            tokens(&child, loop_range, bindings, locals, output);
        }
    } else if kind == "identifier"
        && let Some(declaration) = bindings.get(&node.range().start)
        && loop_range.contains(declaration)
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
    inputs: LoopInputs,
    severity: Option<&'static str>,
    options: LoopSimilarity,
    bypasses: Vec<LoopBypass>,
) -> File {
    let mut loops = Vec::new();
    if severity.is_some() {
        let minimum = options.min_body_tokens.expect("embedded minimum body size");
        for node in root.dfs().filter(|node| {
            matches!(
                node.kind().as_ref(),
                "for_statement" | "for_in_statement" | "while_statement" | "do_statement"
            )
        }) {
            let Some(body) = node.field("body") else {
                continue;
            };
            if body.dfs().any(|node| node.is_error() || node.is_missing()) {
                continue;
            }
            let mut body_tokens = Vec::new();
            tokens(
                &body,
                &node.range(),
                &inputs.bindings,
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
        imports: inputs.imports,
        severity,
        loops,
        bypasses,
    }
}

fn resolve_import(
    root: &Path,
    file: &str,
    specifier: &str,
    ids: &BTreeMap<&str, usize>,
) -> Option<usize> {
    let base = normalized(&root.join(file).with_file_name("").join(specifier));
    let relative = base.strip_prefix(root).ok()?.to_str()?.replace('\\', "/");
    if let Some(id) = ids.get(relative.as_str()) {
        return Some(*id);
    }
    let alternatives: &[(&str, &[&str])] = &[
        (".js", &[".ts", ".tsx", ".jsx"]),
        (".mjs", &[".mts"]),
        (".cjs", &[".cts"]),
    ];
    for (extension, replacements) in alternatives {
        if let Some(stem) = relative.strip_suffix(extension) {
            return replacements
                .iter()
                .find_map(|extension| ids.get(format!("{stem}{extension}").as_str()).copied());
        }
    }
    [
        ".ts",
        ".tsx",
        ".js",
        ".jsx",
        "/index.ts",
        "/index.tsx",
        "/index.js",
        "/index.jsx",
    ]
    .into_iter()
    .find_map(|suffix| ids.get(format!("{relative}{suffix}").as_str()).copied())
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
            for specifier in &file.imports {
                if let Some(target) = resolve_import(root, &file.path, specifier, &ids)
                    .filter(|target| *target != index)
                {
                    graph.outgoing[index].insert(target);
                    graph.neighbors[index].insert(target);
                    graph.neighbors[target].insert(index);
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

use anyhow::Result;
use serde::Serialize;

use crate::{
    config::{Resolved, RuleBypass, glob},
    selection::{File, Selection, Skipped},
};

#[derive(Serialize)]
pub struct Location {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Serialize)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub severity: String,
    pub rule: String,
    pub message: String,
    pub origin: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related: Option<Location>,
    #[serde(skip)]
    pub offset: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bypass {
    pub finding: Finding,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directive_line: Option<usize>,
    pub reason: String,
}

#[derive(Serialize)]
pub struct Directive {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub kind: &'static str,
    pub text: String,
}

#[derive(Serialize)]
pub struct Metric {
    pub file: String,
    pub check: String,
    pub name: String,
    pub value: usize,
    pub unit: &'static str,
}

#[derive(Serialize)]
pub struct CheckStatus {
    pub id: String,
    pub status: &'static str,
    pub files: usize,
    pub reason: Option<String>,
}

#[derive(Default, Serialize)]
pub struct Summary {
    pub selected: usize,
    pub checked: usize,
    pub unchecked: usize,
    pub skipped: usize,
    pub errors: usize,
    pub warnings: usize,
}

#[derive(Serialize)]
pub struct ConfigSource {
    pub defaults: &'static str,
    pub project: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema_version: u32,
    pub root: String,
    pub mode: &'static str,
    pub config: ConfigSource,
    pub files: Vec<File>,
    pub skipped: Vec<Skipped>,
    pub diagnostics: Vec<Finding>,
    pub metrics: Vec<Metric>,
    pub checks: Vec<CheckStatus>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rule_filter: Vec<String>,
    pub bypasses: Vec<Bypass>,
    pub directives: Vec<Directive>,
    pub summary: Summary,
}

pub fn source_location(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .encode_utf16()
        .count()
        + 1;
    (line, column)
}

impl Report {
    pub fn new(root: String, selected: Selection, configuration: &Resolved, list: bool) -> Self {
        Self {
            schema_version: 1,
            root,
            mode: if list { "list" } else { "check" },
            config: ConfigSource {
                defaults: "embedded:checkignore.json",
                project: configuration.project.clone(),
            },
            files: selected.files,
            skipped: selected.skipped,
            diagnostics: Vec::new(),
            metrics: Vec::new(),
            checks: Vec::new(),
            rule_filter: Vec::new(),
            bypasses: Vec::new(),
            directives: Vec::new(),
            summary: Summary::default(),
        }
    }

    pub fn finding(
        &mut self,
        file: &str,
        source: &str,
        offset: usize,
        severity: &str,
        rule: &str,
        message: impl Into<String>,
    ) {
        let (line, column) = source_location(source, offset);
        self.diagnostics.push(Finding {
            file: file.to_owned(),
            line,
            column,
            severity: severity.to_owned(),
            rule: rule.to_owned(),
            message: message.into(),
            origin: "static",
            related: None,
            offset,
        });
    }

    pub fn metric(&mut self, file: &str, check: &str, name: &str, value: usize) {
        self.metrics.push(Metric {
            file: file.to_owned(),
            check: check.to_owned(),
            name: name.to_owned(),
            value,
            unit: "count",
        });
    }

    pub fn external_finding(
        &mut self,
        file: &str,
        (line, column): (usize, usize),
        severity: &str,
        rule: &str,
        message: String,
        origin: &'static str,
    ) {
        self.diagnostics.push(Finding {
            file: file.to_owned(),
            line,
            column,
            severity: severity.to_owned(),
            rule: rule.to_owned(),
            message,
            origin,
            related: None,
            offset: 0,
        });
    }

    pub fn apply_project_bypasses(&mut self, bypasses: &[RuleBypass]) -> Result<()> {
        if bypasses.is_empty() {
            return Ok(());
        }
        let entries = bypasses
            .iter()
            .map(|bypass| {
                let files = bypass
                    .files
                    .iter()
                    .map(|pattern| glob(pattern))
                    .collect::<Result<Vec<_>>>()?;
                Ok((bypass, files))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut retained = Vec::with_capacity(self.diagnostics.len());
        for finding in self.diagnostics.drain(..) {
            if let Some((bypass, _)) = entries.iter().find(|(bypass, files)| {
                bypass.rule == finding.rule
                    && files.iter().any(|file| {
                        file.is_match(&finding.file)
                            || finding
                                .related
                                .as_ref()
                                .is_some_and(|related| file.is_match(&related.file))
                    })
            }) {
                self.bypasses.push(Bypass {
                    finding,
                    directive_line: None,
                    reason: bypass.reason.clone(),
                });
            } else {
                retained.push(finding);
            }
        }
        self.diagnostics = retained;
        Ok(())
    }

    pub fn finish(&mut self) {
        self.summary.selected = self.files.len();
        self.summary.skipped = self.skipped.len();
        self.summary.errors = self
            .diagnostics
            .iter()
            .filter(|finding| finding.severity == "error")
            .count();
        self.summary.warnings = self
            .diagnostics
            .iter()
            .filter(|finding| finding.severity == "warning")
            .count();
        self.diagnostics.sort_by(|a, b| {
            (&a.file, a.line, a.column, &a.rule).cmp(&(&b.file, b.line, b.column, &b.rule))
        });
    }
}

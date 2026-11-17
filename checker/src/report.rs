use serde::Serialize;

use crate::{
    config::Resolved,
    selection::{File, Selection, Skipped},
};

#[derive(Serialize)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub severity: String,
    pub rule: String,
    pub message: String,
    pub origin: &'static str,
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
    pub summary: Summary,
}

impl Report {
    pub fn new(root: String, selected: Selection, configuration: &Resolved, list: bool) -> Self {
        let summary = Summary {
            selected: selected.files.len(),
            skipped: selected.skipped.len(),
            ..Summary::default()
        };
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
            summary,
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
        let offset = offset.min(source.len());
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .encode_utf16()
            .count()
            + 1;
        self.diagnostics.push(Finding {
            file: file.to_owned(),
            line,
            column,
            severity: severity.to_owned(),
            rule: rule.to_owned(),
            message: message.into(),
            origin: "static",
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

    pub fn finish(&mut self) {
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

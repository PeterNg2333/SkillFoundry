use crate::{config::Limits, report::Report};
use serde_json::Value;
use std::collections::BTreeSet;

pub fn json(file: &str, source: &str, report: &mut Report) {
    if let Err(error) = serde_json::from_str::<Value>(source) {
        let start = source
            .split_inclusive('\n')
            .take(error.line().saturating_sub(1))
            .map(str::len)
            .sum::<usize>();
        let offset = start + error.column().saturating_sub(1);
        let offset = (0..=offset.min(source.len()))
            .rev()
            .find(|offset| source.is_char_boundary(*offset))
            .unwrap_or(0);
        report.finding(
            file,
            source,
            offset,
            "error",
            "json/syntax",
            error.to_string(),
        );
    }
}

pub fn toml(file: &str, source: &str, report: &mut Report) {
    if let Err(error) = source.parse::<toml::Table>() {
        let offset = error.span().map_or(0, |span| span.start);
        report.finding(
            file,
            source,
            offset,
            "error",
            "toml/syntax",
            error.to_string(),
        );
    }
}

pub fn dependencies(file: &str, source: &str, limits: &Limits, report: &mut Report) {
    let mut names = BTreeSet::new();
    if file.ends_with("package.json") {
        let Ok(manifest) = serde_json::from_str::<Value>(source) else {
            json(file, source, report);
            return;
        };
        if !manifest.is_object() {
            report.finding(
                file,
                source,
                0,
                "error",
                "dependencies/manifest",
                "Expected a package.json object.",
            );
            return;
        }
        for (group, metric) in [
            ("dependencies", "runtimeDependencies"),
            ("devDependencies", "developmentDependencies"),
            ("peerDependencies", "peerDependencies"),
            ("optionalDependencies", "optionalDependencies"),
        ] {
            let count = match manifest.get(group) {
                None => 0,
                Some(Value::Object(entries)) => {
                    names.extend(entries.keys().cloned());
                    entries.len()
                }
                _ => {
                    report.finding(
                        file,
                        source,
                        0,
                        "error",
                        "dependencies/manifest",
                        format!("{group} must be an object."),
                    );
                    0
                }
            };
            report.metric(file, "dependencies", metric, count);
        }
    } else if file.ends_with("Cargo.toml") {
        let Ok(manifest) = source.parse::<toml::Table>() else {
            toml(file, source, report);
            return;
        };
        let mut scopes = vec![&manifest];
        if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
            scopes.extend(targets.values().filter_map(toml::Value::as_table));
        }
        for (group, metric) in [
            ("dependencies", "runtimeDependencies"),
            ("dev-dependencies", "developmentDependencies"),
            ("build-dependencies", "buildDependencies"),
        ] {
            let mut group_names = BTreeSet::new();
            for scope in &scopes {
                match scope.get(group) {
                    Some(toml::Value::Table(entries)) => {
                        group_names.extend(entries.keys().cloned())
                    }
                    None => {}
                    _ => report.finding(
                        file,
                        source,
                        0,
                        "error",
                        "dependencies/manifest",
                        format!("{group} must be a table."),
                    ),
                }
            }
            report.metric(file, "dependencies", metric, group_names.len());
            names.extend(group_names);
        }
    } else {
        let mut inside = false;
        let mut direct = 0;
        let mut indirect = 0;
        for line in source.lines() {
            let (content, comment) = line.split_once("//").unwrap_or((line, ""));
            let mut content = content.trim();
            if content == ")" {
                inside = false;
                continue;
            }
            if let Some(require) = content
                .strip_prefix("require ")
                .or_else(|| content.strip_prefix("require\t"))
            {
                content = require.trim();
            } else if !inside {
                continue;
            }
            if content == "(" {
                inside = true;
                continue;
            }
            let fields: Vec<_> = content.split_whitespace().collect();
            if fields.is_empty() {
                continue;
            }
            if fields.len() != 2 || !fields[1].starts_with('v') {
                report.finding(
                    file,
                    source,
                    0,
                    "error",
                    "dependencies/manifest",
                    "Malformed go.mod require directive.",
                );
                continue;
            }
            names.insert(fields[0].to_owned());
            if comment.trim() == "indirect" {
                indirect += 1;
            } else {
                direct += 1;
            }
        }
        report.metric(file, "dependencies", "directRequiredModules", direct);
        report.metric(file, "dependencies", "indirectRequiredModules", indirect);
    }
    report.metric(
        file,
        "dependencies",
        "uniqueDeclaredDependencies",
        names.len(),
    );
    if limits
        .direct_dependencies
        .is_some_and(|limit| names.len() > limit)
    {
        report.finding(
            file,
            source,
            0,
            "warning",
            "dependencies/count",
            format!(
                "{} unique declared dependencies exceed the configured limit.",
                names.len()
            ),
        );
    }
}

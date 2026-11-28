//! Rule parity across languages.
//!
//! `capabilities --json` declares which languages implement each native rule. Every
//! `tests/parity/<case>/expect.json` names one rule and the exact lines it reports in each case
//! file. Files left out of `findings` must report nothing. The corpus must show a finding for
//! every declared rule and language, and no case may use a language its rule does not declare.

mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use support::Project;

fn case_files(directory: &Path, prefix: &str, files: &mut Vec<(String, String)>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_str().unwrap();
        let relative = format!("{prefix}{name}");
        if path.is_dir() {
            case_files(&path, &format!("{relative}/"), files);
        } else if relative != "expect.json" {
            files.push((relative, fs::read_to_string(&path).unwrap()));
        }
    }
}

fn lines_by_file(report: &Value, rule: &str) -> BTreeMap<String, Vec<u64>> {
    let mut lines: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for finding in report["diagnostics"].as_array().unwrap() {
        if finding["rule"] == rule {
            lines
                .entry(finding["file"].as_str().unwrap().to_owned())
                .or_default()
                .push(finding["line"].as_u64().unwrap());
        }
    }
    lines
}

#[test]
fn every_declared_rule_language_has_a_matching_parity_case() {
    let capabilities = Project::new(&[]).success(&["capabilities", "--json"], true);
    let declared: BTreeMap<String, BTreeSet<String>> = capabilities["ruleLanguages"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(rule, languages)| {
            let languages = languages
                .as_array()
                .unwrap()
                .iter()
                .map(|language| language.as_str().unwrap().to_owned())
                .collect();
            (rule.clone(), languages)
        })
        .collect();
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/parity");
    let mut cases: Vec<_> = fs::read_dir(&corpus)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    cases.sort();
    let mut demonstrated = BTreeSet::new();
    let mut failures = Vec::new();
    for case in cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let expect: Value =
            serde_json::from_str(&fs::read_to_string(case.join("expect.json")).unwrap()).unwrap();
        let rule = expect["rule"].as_str().unwrap();
        let Some(languages) = declared.get(rule) else {
            failures.push(format!("{name}: {rule} is not a declared rule"));
            continue;
        };
        let mut files = Vec::new();
        case_files(&case, "", &mut files);
        let files: Vec<_> = files
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str()))
            .collect();
        let project = Project::new(&files);
        let (status, report, stderr) = project.run(&["check", "--json", "--rules", rule], true);
        if status == 2 {
            failures.push(format!("{name}: check failed: {stderr}"));
            continue;
        }
        let expected: BTreeMap<String, Vec<u64>> = expect["findings"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(_, lines)| !lines.as_array().unwrap().is_empty())
            .map(|(file, lines)| {
                let lines = lines
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|line| line.as_u64().unwrap())
                    .collect();
                (file.clone(), lines)
            })
            .collect();
        let actual = lines_by_file(&report, rule);
        if actual != expected {
            failures.push(format!(
                "{name}: {rule} expected {expected:?}, found {actual:?}"
            ));
        }
        for file in report["files"].as_array().unwrap() {
            let (path, language) = (
                file["path"].as_str().unwrap(),
                file["type"].as_str().unwrap(),
            );
            if !declared
                .values()
                .any(|languages| languages.contains(language))
            {
                continue;
            }
            if !languages.contains(language) {
                failures.push(format!(
                    "{name}: {path} is {language}, which {rule} does not declare"
                ));
            } else if expected.contains_key(path) {
                demonstrated.insert((rule.to_owned(), language.to_owned()));
            }
        }
    }
    for (rule, languages) in &declared {
        for language in languages {
            if !demonstrated.contains(&(rule.clone(), language.clone())) {
                failures.push(format!(
                    "missing parity case: {rule} for {language} (add a flagged file under tests/parity/)"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

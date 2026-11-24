mod structure;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result, bail, ensure};
use globset::{GlobBuilder, GlobMatcher};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub use structure::{FileStructure, FileStructureIgnore, FileStructurePattern};

pub const SINGLE_USE_HELPER: &str = "maintainable/single-use-helper";
pub const OBJECT_SPREAD_NEWLINE: &str = "maintainable/object-spread-newline";
pub const LONG_CONDITION: &str = "maintainable/long-condition";
pub const FALLBACK_ORDER: &str = "maintainable/fallback-order";
pub const LONG_JSX_CLASS_NAME: &str = "maintainable/long-jsx-class-name";
pub const PLAYWRIGHT_STYLE_ASSERTION: &str = "maintainable/playwright-style-assertion";
pub const COMPLEX_MAPPED_BUTTON: &str = "maintainable/complex-mapped-button";
pub const SIMILAR_LOOPS: &str = "maintainable/similar-loops";
pub const TOOL_CHECKS: &[&str] = &[
    "dependency-boundaries",
    "duplication",
    "python",
    "go-format",
];

pub const RULES: &[&str] = &[
    "prefer-const",
    "no-var",
    "func-style",
    "no-restricted-syntax",
    "maintainable/cross-directory-imports",
    "@typescript-eslint/consistent-type-imports",
    "max-depth",
    "complexity",
    "max-lines",
    SINGLE_USE_HELPER,
    OBJECT_SPREAD_NEWLINE,
    LONG_CONDITION,
    FALLBACK_ORDER,
    LONG_JSX_CLASS_NAME,
    PLAYWRIGHT_STYLE_ASSERTION,
    COMPLEX_MAPPED_BUTTON,
    SIMILAR_LOOPS,
];

pub fn validate_rule_filter(rules: &[String]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for rule in rules {
        ensure!(RULES.contains(&rule.as_str()), "unknown rule {rule}");
        ensure!(seen.insert(rule), "duplicate rule {rule}");
    }
    Ok(())
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Limits {
    pub direct_dependencies: Option<usize>,
    pub imports_per_file: Option<usize>,
    pub require_comments: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    pub files: Vec<String>,
    pub rules: BTreeMap<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuleBypass {
    pub files: Vec<String>,
    pub rule: String,
    pub reason: String,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckSeverity {
    Warn,
    #[default]
    Error,
}

impl CheckSeverity {
    pub fn diagnostic(self) -> &'static str {
        match self {
            Self::Warn => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportBoundary {
    pub name: String,
    pub from: Option<String>,
    pub to: Option<String>,
    #[serde(default)]
    pub circular: bool,
    #[serde(default)]
    pub severity: CheckSeverity,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct DependencyBoundaries {
    pub ts_config: Option<String>,
    pub forbidden: Vec<ImportBoundary>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Duplication {
    pub min_tokens: Option<usize>,
    pub min_lines: Option<usize>,
    pub severity: Option<CheckSeverity>,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct LoopSimilarity {
    pub min_similarity: Option<usize>,
    pub min_body_tokens: Option<usize>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Settings {
    pub ignore: Vec<String>,
    pub extensions: BTreeMap<String, Value>,
    pub rules: BTreeMap<String, Value>,
    pub file_structure: Option<FileStructure>,
    pub overrides: Vec<Override>,
    pub style_notes: Vec<String>,
    pub bypasses: Vec<RuleBypass>,
    pub external_checks: Vec<String>,
    pub dependency_boundaries: DependencyBoundaries,
    pub python_rules: Option<Vec<String>>,
    pub duplication: Duplication,
    pub loop_similarity: LoopSimilarity,
    pub limits: Limits,
}

pub struct Resolved {
    pub settings: Settings,
    pub project: Option<String>,
}

pub fn glob(pattern: &str) -> Result<GlobMatcher> {
    Ok(GlobBuilder::new(pattern)
        .literal_separator(true)
        .backslash_escape(true)
        .build()?
        .compile_matcher())
}

fn validate_rules(rules: &BTreeMap<String, Value>) -> Result<()> {
    for (name, value) in rules {
        ensure!(RULES.contains(&name.as_str()), "unsupported rule {name}");
        let severity = value
            .as_array()
            .and_then(|values| values.first())
            .unwrap_or(value);
        ensure!(
            matches!(severity.as_str(), Some("off" | "warn" | "error"))
                || matches!(severity.as_u64(), Some(0..=2)),
            "invalid severity for {name}"
        );
        if let Some(options) = value.as_array() {
            ensure!(
                !options.is_empty() && options.len() <= 2,
                "invalid options for {name}"
            );
            if let Some(option) = options.get(1) {
                match name.as_str() {
                    "max-depth" | "complexity" => {
                        ensure!(option.as_u64().is_some(), "invalid threshold for {name}")
                    }
                    "max-lines" => {
                        ensure!(
                            option.as_u64().is_some() || option.is_object(),
                            "invalid options for max-lines"
                        );
                        if let Some(object) = option.as_object() {
                            for (key, value) in object {
                                match key.as_str() {
                                    "max" => {
                                        ensure!(value.as_u64().is_some(), "invalid max-lines limit")
                                    }
                                    "skipBlankLines" | "skipComments" => ensure!(
                                        value.is_boolean(),
                                        "invalid max-lines option {key}"
                                    ),
                                    _ => bail!("unsupported max-lines option {key}"),
                                }
                            }
                        }
                    }
                    "func-style" => ensure!(
                        option == "expression",
                        "only expression func-style is supported"
                    ),
                    _ => bail!("unsupported options for {name}"),
                }
            }
        }
    }
    Ok(())
}

fn lint_code(code: &str) -> bool {
    !code.is_empty()
        && code.as_bytes()[0].is_ascii_uppercase()
        && code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

fn validate_external(settings: &Settings) -> Result<()> {
    let mut checks = BTreeSet::new();
    for check in &settings.external_checks {
        ensure!(
            TOOL_CHECKS.contains(&check.as_str()),
            "unknown external check {check}"
        );
        ensure!(checks.insert(check), "duplicate external check {check}");
    }
    if let Some(rules) = &settings.python_rules {
        ensure!(
            !rules.is_empty() && rules.iter().all(|rule| lint_code(rule)),
            "pythonRules requires Ruff rule codes or prefixes"
        );
    }
    ensure!(
        settings.duplication.min_lines != Some(0) && settings.duplication.min_tokens != Some(0),
        "duplication thresholds must be positive"
    );
    let mut names = BTreeSet::new();
    for rule in &settings.dependency_boundaries.forbidden {
        ensure!(
            !rule.name.is_empty()
                && rule
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
            "invalid dependency boundary name {}",
            rule.name
        );
        ensure!(
            names.insert(&rule.name),
            "duplicate dependency boundary {}",
            rule.name
        );
        ensure!(
            rule.to.is_some() || rule.circular,
            "dependency boundary {} requires to or circular",
            rule.name
        );
        ensure!(
            rule.from.as_ref().is_none_or(|pattern| !pattern.is_empty())
                && rule.to.as_ref().is_none_or(|pattern| !pattern.is_empty()),
            "dependency boundary paths must be nonempty regex patterns"
        );
    }
    Ok(())
}

fn bypass_rule(settings: &Settings, rule: &str) -> bool {
    RULES.contains(&rule)
        || matches!(rule, "jscpd/duplicate" | "gofmt/format")
        || rule.strip_prefix("ruff/").is_some_and(|code| {
            lint_code(code) && code.ends_with(|character: char| character.is_ascii_digit())
        })
        || rule
            .strip_prefix("dependency-cruiser/")
            .is_some_and(|name| {
                settings
                    .dependency_boundaries
                    .forbidden
                    .iter()
                    .any(|rule| rule.name == name)
            })
}

fn decode(source: &str, name: &str) -> Result<Settings> {
    let settings: Settings = serde_json::from_str(source.trim_start_matches('\u{feff}'))
        .with_context(|| format!("configuration {name}"))?;
    let validate = || -> Result<()> {
        for pattern in &settings.ignore {
            ensure!(
                !pattern.is_empty() && !pattern.starts_with('!'),
                "use positive ignore globs"
            );
            glob(pattern)?;
        }
        for (extension, kind) in &settings.extensions {
            ensure!(
                extension.starts_with('.') && !extension.contains(['/', '\\']),
                "unsupported extension mapping {extension}"
            );
            ensure!(
                kind == false
                    || matches!(
                        kind.as_str(),
                        Some(
                            "javascript"
                                | "typescript"
                                | "python"
                                | "json"
                                | "go"
                                | "gomod"
                                | "toml"
                        )
                    ),
                "unsupported extension mapping {extension}"
            );
        }
        validate_rules(&settings.rules)?;
        validate_external(&settings)?;
        if let Some(structure) = &settings.file_structure {
            structure.validate().context("fileStructure")?;
        }
        ensure!(
            settings
                .loop_similarity
                .min_similarity
                .is_none_or(|value| (1..=100).contains(&value)),
            "loopSimilarity.minSimilarity must be between 1 and 100"
        );
        ensure!(
            settings
                .loop_similarity
                .min_body_tokens
                .is_none_or(|value| value >= 3),
            "loopSimilarity.minBodyTokens must be at least 3"
        );
        ensure!(
            settings
                .style_notes
                .iter()
                .all(|note| !note.trim().is_empty()),
            "styleNotes requires nonempty notes"
        );
        for bypass in &settings.bypasses {
            ensure!(
                bypass_rule(&settings, &bypass.rule),
                "unsupported bypass rule {}",
                bypass.rule
            );
            ensure!(!bypass.reason.trim().is_empty(), "bypass requires a reason");
            ensure!(!bypass.files.is_empty(), "bypass requires files");
            for pattern in &bypass.files {
                ensure!(
                    !pattern.trim().is_empty() && !pattern.starts_with('!'),
                    "use positive bypass globs"
                );
                glob(pattern)?;
            }
        }
        for rule_override in &settings.overrides {
            ensure!(!rule_override.files.is_empty(), "override requires files");
            for pattern in &rule_override.files {
                glob(pattern)?;
            }
            validate_rules(&rule_override.rules)?;
        }
        Ok(())
    };
    validate().with_context(|| format!("configuration {name}"))?;
    Ok(settings)
}

pub fn load(root: &Path, requested: Option<&Path>) -> Result<Resolved> {
    let mut settings = decode(
        include_str!("../config/defaults.json"),
        "embedded:checkignore.json",
    )?;
    let mut file = root.join(requested.unwrap_or(Path::new("checkignore.json")));
    let mut source = fs::read_to_string(&file);
    if requested.is_none()
        && source
            .as_ref()
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    {
        file = root.join("check.json");
        source = fs::read_to_string(&file);
    }
    let source = match source {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && requested.is_none() => {
            return Ok(Resolved {
                settings,
                project: None,
            });
        }
        Err(error) => {
            return Err(error).with_context(|| format!("read configuration {}", file.display()));
        }
    };
    let project = decode(&source, &file.display().to_string())?;
    settings.ignore.extend(project.ignore);
    settings.extensions.extend(
        project
            .extensions
            .into_iter()
            .map(|(extension, kind)| (extension.to_lowercase(), kind)),
    );
    settings.rules.extend(project.rules);
    settings.file_structure = project.file_structure;
    settings.overrides.extend(project.overrides);
    settings.style_notes.extend(project.style_notes);
    settings.bypasses.extend(project.bypasses);
    settings.external_checks.extend(project.external_checks);
    settings.dependency_boundaries = project.dependency_boundaries;
    settings.python_rules = project.python_rules.or(settings.python_rules);
    settings.loop_similarity.min_similarity = project
        .loop_similarity
        .min_similarity
        .or(settings.loop_similarity.min_similarity);
    settings.loop_similarity.min_body_tokens = project
        .loop_similarity
        .min_body_tokens
        .or(settings.loop_similarity.min_body_tokens);
    settings.duplication.min_tokens = project
        .duplication
        .min_tokens
        .or(settings.duplication.min_tokens);
    settings.duplication.min_lines = project
        .duplication
        .min_lines
        .or(settings.duplication.min_lines);
    settings.duplication.severity = project
        .duplication
        .severity
        .or(settings.duplication.severity);
    settings.limits = project.limits;
    Ok(Resolved {
        settings,
        project: Some(file.display().to_string()),
    })
}

pub struct Policy<'a> {
    rules: BTreeMap<String, Value>,
    filter: &'a [String],
}

impl<'a> Policy<'a> {
    pub fn for_file(settings: &Settings, path: &str, filter: &'a [String]) -> Result<Self> {
        let mut rules: BTreeMap<_, _> = RULES
            .iter()
            .map(|name| (name.to_string(), json!("error")))
            .collect();
        rules.extend(settings.rules.clone());
        for rule_override in &settings.overrides {
            if rule_override
                .files
                .iter()
                .map(|pattern| glob(pattern))
                .collect::<Result<Vec<_>>>()?
                .iter()
                .any(|matcher| matcher.is_match(path))
            {
                rules.extend(rule_override.rules.clone());
            }
        }
        Ok(Self { rules, filter })
    }

    pub fn severity(&self, name: &str) -> Option<&'static str> {
        if !self.filter.is_empty() && !self.filter.iter().any(|rule| rule == name) {
            return None;
        }
        rule_severity(self.rules.get(name)?)
    }

    pub fn option(&self, name: &str) -> Option<&Value> {
        self.rules.get(name)?.as_array()?.get(1)
    }

    pub fn threshold(&self, name: &str, default: u64) -> u64 {
        self.option(name).and_then(Value::as_u64).unwrap_or(default)
    }
}

pub fn rule_severity(value: &Value) -> Option<&'static str> {
    let severity = value
        .as_array()
        .and_then(|values| values.first())
        .unwrap_or(value);
    match (severity.as_str(), severity.as_u64()) {
        (Some("error"), _) | (_, Some(2)) => Some("error"),
        (Some("warn"), _) | (_, Some(1)) => Some("warning"),
        _ => None,
    }
}

use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, bail, ensure};
use globset::{GlobBuilder, GlobMatcher};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const RULES: &[&str] = &[
    "prefer-const",
    "no-var",
    "func-style",
    "no-restricted-syntax",
    "maintainable/cross-directory-imports",
    "@typescript-eslint/consistent-type-imports",
    "max-depth",
    "complexity",
    "max-lines",
];

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

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub ignore: Vec<String>,
    pub extensions: BTreeMap<String, Value>,
    pub rules: BTreeMap<String, Value>,
    pub overrides: Vec<Override>,
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
                        Some("javascript" | "typescript" | "json" | "go" | "gomod" | "toml")
                    ),
                "unsupported extension mapping {extension}"
            );
        }
        validate_rules(&settings.rules)?;
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
    let file = root.join(requested.unwrap_or(Path::new("checkignore.json")));
    let source = match fs::read_to_string(&file) {
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
    settings.overrides.extend(project.overrides);
    settings.limits = project.limits;
    Ok(Resolved {
        settings,
        project: Some(file.display().to_string()),
    })
}

pub struct Policy(BTreeMap<String, Value>);

impl Policy {
    pub fn for_file(settings: &Settings, path: &str) -> Result<Self> {
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
        Ok(Self(rules))
    }

    pub fn severity(&self, name: &str) -> Option<&str> {
        let value = self.0.get(name)?;
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

    pub fn option(&self, name: &str) -> Option<&Value> {
        self.0.get(name)?.as_array()?.get(1)
    }

    pub fn threshold(&self, name: &str, default: u64) -> u64 {
        self.option(name).and_then(Value::as_u64).unwrap_or(default)
    }
}

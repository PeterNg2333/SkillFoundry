use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    env, fs,
    path::Path,
    process::{Command, Output},
};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::json;

use crate::{
    config::Settings,
    report::{CheckStatus, Report},
    selection::File,
};

pub fn run(
    check: &str,
    root: &Path,
    settings: &Settings,
    report: &mut Report,
    checked: &mut BTreeSet<String>,
) -> Result<()> {
    let (tool, npm_entry) = match check {
        "dependency-boundaries" => ("dependency-cruiser", Some("bin/dependency-cruiser.mjs")),
        "duplication" => ("jscpd", Some("run-jscpd.js")),
        "python-lint" => ("ruff", None),
        "go-format" => ("gofmt", None),
        _ => unreachable!(),
    };
    let files: Vec<_> = report
        .files
        .iter()
        .filter(|file| applies(check, file))
        .map(|file| file.path.clone())
        .collect();
    let mut status = CheckStatus {
        id: check.to_owned(),
        status: "not-applicable",
        files: 0,
        reason: None,
    };
    if !files.is_empty() {
        if check == "dependency-boundaries" && settings.dependency_boundaries.forbidden.is_empty() {
            status.status = "unsupported";
            status.reason = Some(
                "Declare dependencyBoundaries.forbidden in the project configuration.".to_owned(),
            );
        } else if let Some(mut command) = tool_command(tool, npm_entry) {
            let input = ToolInput {
                root,
                files: &files,
                settings,
            };
            let scan = match check {
                "dependency-boundaries" => dependencies(&mut command, &input, report),
                "duplication" => duplicates(&mut command, &input, report),
                "python-lint" => python(&command, &input, report),
                "go-format" => go_format(&command, &input, report),
                _ => unreachable!(),
            }
            .with_context(|| format!("run {check} with {tool}"))?;
            status.files = scan.count;
            status.status = if scan.count == files.len() {
                "checked"
            } else {
                "partial"
            };
            if status.status == "partial" {
                status.reason = Some(format!(
                    "{tool} analyzed {} of {} selected files.",
                    scan.count,
                    files.len()
                ));
            }
            checked.extend(scan.covered);
        } else {
            status.status = "unsupported";
            status.reason = Some(format!("Install {tool} and put it on PATH to run {check}."));
        }
    }
    report.checks.push(status);
    Ok(())
}

fn applies(check: &str, file: &File) -> bool {
    let ecmascript = matches!(file.kind.as_str(), "javascript" | "typescript");
    match check {
        "dependency-boundaries" => ecmascript,
        "duplication" => ecmascript || matches!(file.kind.as_str(), "python" | "go"),
        "python-lint" => file.kind == "python",
        "go-format" => file.kind == "go",
        _ => false,
    }
}

struct ToolInput<'a> {
    root: &'a Path,
    files: &'a [String],
    settings: &'a Settings,
}

struct Scan {
    covered: Vec<String>,
    count: usize,
}

impl Scan {
    fn complete(files: &[String]) -> Self {
        Self {
            covered: files.to_vec(),
            count: files.len(),
        }
    }
}

fn tool_command(name: &str, npm_entry: Option<&str>) -> Option<Command> {
    let path = env::var_os("PATH")?;
    let directories = env::split_paths(&path).filter(|directory| directory.is_absolute());
    for directory in directories {
        let executable = directory.join(format!("{name}{}", env::consts::EXE_SUFFIX));
        if executable.is_file() {
            return Some(Command::new(executable));
        }
        if cfg!(windows)
            && let Some(entry) = npm_entry
        {
            // Resolve npm's package entry directly so project paths never go through cmd.exe.
            for modules in [
                directory.join("node_modules"),
                directory.parent().unwrap_or(&directory).to_path_buf(),
            ] {
                let script = modules.join(name).join(entry);
                if script.is_file() {
                    let mut command = tool_command("node", None)?;
                    command.arg(script);
                    return Some(command);
                }
            }
        }
    }
    None
}

fn run_tool(command: &mut Command, success: &[i32]) -> Result<Output> {
    let output = command.output()?;
    ensure!(
        output
            .status
            .code()
            .is_some_and(|code| success.contains(&code)),
        "process exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output)
}

fn normalized_name(base: &Path, name: &str) -> String {
    let path = Path::new(name);
    let relative = path
        .strip_prefix(base)
        .or_else(|_| path.strip_prefix("."))
        .unwrap_or(path);
    relative.to_string_lossy().replace('\\', "/")
}

fn exact_paths(files: &[String]) -> String {
    let mut paths = String::from("^(?:");
    for (index, file) in files.iter().enumerate() {
        if index > 0 {
            paths.push('|');
        }
        for character in file.chars() {
            if r"\.^$|?*+()[]{}".contains(character) {
                paths.push('\\');
            }
            paths.push(character);
        }
    }
    paths.push_str(")$");
    paths
}

#[derive(Deserialize)]
struct DependencyReport {
    modules: Vec<DependencyModule>,
    summary: DependencySummary,
}

#[derive(Deserialize)]
struct DependencyModule {
    source: String,
    followable: Option<bool>,
}

#[derive(Deserialize)]
struct DependencySummary {
    violations: Vec<DependencyViolation>,
}

#[derive(Deserialize)]
struct DependencyViolation {
    from: String,
    to: String,
    rule: DependencyRule,
}

#[derive(Deserialize)]
struct DependencyRule {
    name: String,
}

fn dependencies(command: &mut Command, input: &ToolInput<'_>, report: &mut Report) -> Result<Scan> {
    let settings = &input.settings.dependency_boundaries;
    let mut forbidden = Vec::new();
    for rule in &settings.forbidden {
        let mut from = json!({});
        let mut to = json!({});
        if let Some(path) = &rule.from {
            from["path"] = json!(path);
        }
        if let Some(path) = &rule.to {
            to["path"] = json!(path);
        }
        if rule.circular {
            to["circular"] = json!(true);
        }
        // SkillFoundry applies severity after policy bypasses.
        forbidden.push(json!({"name":rule.name, "severity":"warn", "from":from, "to":to}));
    }
    let mut options = json!({
        "includeOnly": exact_paths(input.files),
        "tsPreCompilationDeps": true
    });
    if let Some(config) = &settings.ts_config {
        let full = dunce::canonicalize(input.root.join(config))
            .context("resolve dependencyBoundaries.tsConfig")?;
        ensure!(
            Path::new(config).is_relative()
                && full.starts_with(input.root)
                && full
                    .extension()
                    .is_some_and(|extension| extension == "json")
                && full.is_file(),
            "dependencyBoundaries.tsConfig must be a JSON file inside the project"
        );
        options["tsConfig"] = json!({"fileName": full});
    }
    let work = tempfile::tempdir()?;
    let config = work.path().join("boundaries.json");
    fs::write(
        &config,
        serde_json::to_vec(&json!({"forbidden":forbidden,"options":options}))?,
    )?;
    let output = run_tool(
        command
            .current_dir(input.root)
            .arg("--config")
            .arg(config)
            .args(["--output-type", "json", "."]),
        &[0],
    )?;
    let dependencies: DependencyReport =
        serde_json::from_slice(&output.stdout).context("parse dependency-cruiser JSON")?;
    let selected: HashSet<_> = input.files.iter().map(String::as_str).collect();
    let covered: BTreeSet<_> = dependencies
        .modules
        .into_iter()
        .filter(|module| module.followable != Some(false))
        .map(|module| normalized_name(input.root, &module.source))
        .filter(|file| selected.contains(file.as_str()))
        .collect();
    for violation in dependencies.summary.violations {
        let from = normalized_name(input.root, &violation.from);
        let to = normalized_name(input.root, &violation.to);
        if covered.contains(&from)
            && covered.contains(&to)
            && let Some(rule) = settings
                .forbidden
                .iter()
                .find(|rule| rule.name == violation.rule.name)
        {
            report.external_finding(
                &from,
                (1, 1),
                rule.severity.diagnostic(),
                &format!("dependency-cruiser/{}", rule.name),
                format!("Dependency on {to} violates {}.", rule.name),
                "dependency-cruiser",
            );
        }
    }
    Ok(Scan {
        count: covered.len(),
        covered: covered.into_iter().collect(),
    })
}

#[derive(Deserialize)]
struct DuplicateReport {
    duplicates: Vec<Duplicate>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Duplicate {
    lines: usize,
    first_file: DuplicateBlock,
    second_file: DuplicateBlock,
}

#[derive(Deserialize)]
struct DuplicateBlock {
    name: String,
    start: usize,
}

fn duplicates(command: &mut Command, input: &ToolInput<'_>, report: &mut Report) -> Result<Scan> {
    let work = tempfile::tempdir()?;
    let sources = work.path().join("sources");
    let reports = work.path().join("reports");
    let config = work.path().join("jscpd.json");
    fs::write(&config, b"{}")?;
    let mut covered = Vec::new();
    let mut extensions: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
    let mut max_size = 1;
    for file in input.files {
        let Some((format, extension)) = jscpd_format(file) else {
            continue;
        };
        extensions.entry(format).or_default().insert(extension);
        let destination = sources.join(file);
        fs::create_dir_all(
            destination
                .parent()
                .context("staged source has no parent")?,
        )?;
        let size = fs::copy(input.root.join(file), destination)?;
        max_size = max_size.max(size.saturating_add(1));
        covered.push(file.clone());
    }
    if covered.is_empty() {
        return Ok(Scan::complete(&covered));
    }
    let formats = extensions.keys().copied().collect::<Vec<_>>().join(",");
    let extension_mappings = extensions
        .into_iter()
        .map(|(format, extensions)| {
            format!(
                "{format}:{}",
                extensions.into_iter().collect::<Vec<_>>().join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    let settings = &input.settings.duplication;
    command
        .current_dir(&sources)
        .arg("--config")
        .arg(config)
        .args(["--no-gitignore", "--reporters", "json", "--format"])
        .arg(formats)
        .arg("--formats-exts")
        .arg(extension_mappings)
        .arg("--max-size")
        .arg(max_size.to_string())
        .arg("--output")
        .arg(&reports);
    if let Some(lines) = settings.min_lines {
        command.arg("--min-lines").arg(lines.to_string());
    }
    if let Some(tokens) = settings.min_tokens {
        command.arg("--min-tokens").arg(tokens.to_string());
    }
    run_tool(command.arg("."), &[0])?;
    let duplicates: DuplicateReport =
        serde_json::from_slice(&fs::read(reports.join("jscpd-report.json"))?)
            .context("parse jscpd JSON")?;
    let selected: HashSet<_> = covered.iter().map(String::as_str).collect();
    for mut duplicate in duplicates.duplicates {
        duplicate.first_file.name = normalized_name(&sources, &duplicate.first_file.name);
        duplicate.second_file.name = normalized_name(&sources, &duplicate.second_file.name);
        let mut blocks = [duplicate.first_file, duplicate.second_file];
        blocks.sort_by(|a, b| (&a.name, a.start).cmp(&(&b.name, b.start)));
        let [first, second] = blocks;
        if selected.contains(first.name.as_str()) && selected.contains(second.name.as_str()) {
            report.external_finding(
                &first.name,
                (first.start, 1),
                settings.severity.unwrap_or_default().diagnostic(),
                "jscpd/duplicate",
                format!(
                    "{} duplicated lines also appear at {}:{}.",
                    duplicate.lines, second.name, second.start
                ),
                "jscpd",
            );
        }
    }
    Ok(Scan {
        count: covered.len(),
        covered,
    })
}

fn jscpd_format(file: &str) -> Option<(&'static str, &str)> {
    let extension = Path::new(file).extension()?.to_str()?;
    let format = match extension.to_ascii_lowercase().as_str() {
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "py" | "pyi" => "python",
        "go" => "go",
        _ => return None,
    };
    Some((format, extension))
}

#[derive(Deserialize)]
struct RuffFinding {
    filename: String,
    location: RuffLocation,
    code: Option<String>,
    message: String,
}

#[derive(Deserialize)]
struct RuffLocation {
    row: usize,
    column: usize,
}

fn python(command: &Command, input: &ToolInput<'_>, report: &mut Report) -> Result<Scan> {
    let rules = input
        .settings
        .python_rules
        .as_ref()
        .context("embedded pythonRules are missing")?
        .join(",");
    for files in input.files.chunks(16) {
        let mut batch = Command::new(command.get_program());
        let output = run_tool(
            batch
                .current_dir(input.root)
                .args([
                    "check",
                    "--isolated",
                    "--no-cache",
                    "--ignore-noqa",
                    "--output-format",
                    "json",
                    "--select",
                ])
                .arg(&rules)
                .arg("--")
                .args(files),
            &[0, 1],
        )?;
        let diagnostics: Vec<RuffFinding> =
            serde_json::from_slice(&output.stdout).context("parse Ruff JSON")?;
        for finding in diagnostics {
            let file = normalized_name(input.root, &finding.filename);
            if files.contains(&file) {
                report.external_finding(
                    &file,
                    (finding.location.row, finding.location.column),
                    "error",
                    &format!("ruff/{}", finding.code.as_deref().unwrap_or("syntax")),
                    finding.message,
                    "ruff",
                );
            }
        }
    }
    Ok(Scan::complete(input.files))
}

fn go_format(command: &Command, input: &ToolInput<'_>, report: &mut Report) -> Result<Scan> {
    for file in input.files {
        let mut formatter = Command::new(command.get_program());
        let output = run_tool(
            formatter
                .current_dir(input.root)
                .args(["-l", "--"])
                .arg(file),
            &[0, 2],
        )?;
        if !output.status.success() {
            report.external_finding(
                file,
                (1, 1),
                "error",
                "gofmt/syntax",
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                "gofmt",
            );
        } else if !output.stdout.is_empty() {
            report.external_finding(
                file,
                (1, 1),
                "error",
                "gofmt/format",
                "Format this file with gofmt.".to_owned(),
                "gofmt",
            );
        }
    }
    Ok(Scan::complete(input.files))
}

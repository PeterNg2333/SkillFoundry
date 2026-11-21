mod checks;
mod config;
mod report;
mod selection;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(version, about = "Standalone native source-policy checker")]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Capabilities,
    Select {
        paths: Vec<PathBuf>,
    },
    Check {
        #[arg(long, value_delimiter = ',', help = "Check groups from capabilities")]
        checks: Vec<String>,
        #[arg(
            long,
            value_delimiter = ',',
            conflicts_with = "list",
            help = "Exact native JS/TS policy rule IDs from capabilities"
        )]
        rules: Vec<String>,
        #[arg(long)]
        list: bool,
        paths: Vec<PathBuf>,
    },
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    if matches!(cli.command, Command::Capabilities) {
        writeln!(io::stdout().lock(), "{}", checks::capabilities())?;
        return Ok(0);
    }
    let root = dunce::canonicalize(&cli.root).context("resolve check root")?;
    let settings = config::load(&root, cli.config.as_deref())?;
    let (paths, list, names, rules) = match cli.command {
        Command::Select { paths } => (paths, true, Vec::new(), Vec::new()),
        Command::Check {
            paths,
            list,
            mut checks,
            rules,
        } => {
            if checks.is_empty() {
                let defaults = if rules.is_empty() {
                    checks::DEFAULT_CHECKS
                } else {
                    &["javascript", "typescript"]
                };
                checks.extend(defaults.iter().map(|name| (*name).to_owned()));
                if rules.is_empty() {
                    checks.extend(settings.settings.external_checks.iter().cloned());
                }
            }
            (paths, list, checks, rules)
        }
        Command::Capabilities => unreachable!(),
    };
    let selected = selection::select(&root, &paths, &settings)?;
    let mut result = report::Report::new(root.display().to_string(), selected, &settings, list);
    if !list {
        checks::run(&root, &settings, &names, &rules, &mut result)?;
        result.apply_project_bypasses(&settings.settings.bypasses)?;
    }
    result.rule_filter = rules;
    result.finish();
    let mut output = io::stdout().lock();
    if cli.json {
        writeln!(output, "{}", serde_json::to_string_pretty(&result)?)?;
    } else {
        if list {
            for file in &result.files {
                writeln!(output, "{} {}", file.kind, file.path)?;
            }
        }
        for finding in &result.diagnostics {
            writeln!(
                output,
                "{}:{}:{} {} {} {}",
                finding.file,
                finding.line,
                finding.column,
                finding.severity,
                finding.rule,
                finding.message
            )?;
        }
        for bypass in &result.bypasses {
            let finding = &bypass.finding;
            let declared_at = match bypass.directive_line {
                Some(line) => format!("at line {line}"),
                None => "in project configuration".to_owned(),
            };
            writeln!(
                output,
                "{}:{}:{} bypassed {} {declared_at}: {}",
                finding.file, finding.line, finding.column, finding.rule, bypass.reason
            )?;
        }
        for directive in &result.directives {
            writeln!(
                output,
                "{}:{}:{} @{} {}",
                directive.file, directive.line, directive.column, directive.kind, directive.text
            )?;
        }
        for metric in &result.metrics {
            writeln!(
                output,
                "{} {} {} {}",
                metric.file, metric.name, metric.value, metric.unit
            )?;
        }
        for check in result
            .checks
            .iter()
            .filter(|check| matches!(check.status, "unsupported" | "partial"))
        {
            writeln!(
                output,
                "{} {}: {}",
                check.id,
                check.status,
                check.reason.as_deref().unwrap_or("")
            )?;
        }
        writeln!(
            output,
            "{} selected, {} checked, {} unchecked, {} paths skipped, {} errors, {} warnings",
            result.summary.selected,
            result.summary.checked,
            result.summary.unchecked,
            result.summary.skipped,
            result.summary.errors,
            result.summary.warnings
        )?;
    }
    if result
        .checks
        .iter()
        .any(|check| names.contains(&check.id) && matches!(check.status, "unsupported" | "partial"))
    {
        Ok(2)
    } else {
        Ok(i32::from(result.summary.errors > 0))
    }
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("Static check failed: {error:#}");
            std::process::exit(2);
        }
    }
}

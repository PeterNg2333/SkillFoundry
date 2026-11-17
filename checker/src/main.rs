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
        #[arg(long, value_delimiter = ',')]
        checks: Vec<String>,
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
    let (paths, list, names) = match cli.command {
        Command::Select { paths } => (paths, true, Vec::new()),
        Command::Check {
            paths,
            list,
            checks,
        } => (
            paths,
            list,
            if checks.is_empty() {
                checks::DEFAULT_CHECKS
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect()
            } else {
                checks
            },
        ),
        Command::Capabilities => unreachable!(),
    };
    let selected = selection::select(&root, &paths, &settings)?;
    let mut result = report::Report::new(root.display().to_string(), selected, &settings, list);
    if !list {
        checks::run(&root, &settings, &names, &mut result)?;
    }
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
            .filter(|check| check.status == "unsupported")
        {
            writeln!(
                output,
                "{} unsupported: {}",
                check.id,
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
    Ok(i32::from(result.summary.errors > 0))
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

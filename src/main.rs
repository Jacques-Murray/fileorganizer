mod classifier;
mod cli;
mod config;
mod error;
mod executor;
mod journal;
mod planner;
mod util;

use std::path::PathBuf;
use std::process:ExitCode;

use anyhow::{Context,Result};
use clap::Parser;
use colored::Colorize;
use serde::Serialize;
use tabled::settings::Style;
use tabled::{Table,Tabled};

use crate::cli::{Cli, Command, ScanArgs};
use crate::config::Config;
use crate::error::FoError;
use crate::journal::Failure;
use crate::planner::{build_plan, Plan, PlanInput};
use crate::util::{absolutize,human_size};

const EXIT_OK: u8 = 0;
const EXIT_RUNTIME: u8 = 1;
const EXIT_PARTIAL: u8 = 3;

fn main() -> ExitCode {
    // Usage errors exit with code 2 inside clap.
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("{} {:#}", "error:".red().bold(),err);
            ExitCode::from(EXIT_RUNTIME)
        }
    }
}

fn run(cli: Cli) ->  Result<u8> {
    match cli.command {
        Command::Init { force } => cmd_init(force),
        Command::Scan(args) => cmd_scan(args, false),
        Command::Apply(args) => cmd_scan(args, true),
        Command::Undo {
            journal,
            last,
            config,
        }=>cmd_undo(journal,last,config),
        Command::Config {path, config} => cmd_config(path, config),
    }
}

fn cmd_init(force:bool) -> Result<u8> {
    let path = config::default_config_path()?;
    if path.exists() && !force {
        return Err(FoError::ConfigExists(path).into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(&path, config::DEFAULT_CONFIG)
    .with_context(|| format!("writing {}", path.display()))?;
    println!("{} {}", "Created".green(), path.display());
    Ok(EXIT_OK)
}

#[derive(Tabled)]
struct Row {
    #[tabled(rename = "Rule")]
    rule: String,
    #[tabled(rename = "Size")]
    size: String,
    #[tabled(rename = "From")]
    from: String,
    #[tabled(rename = "To")]
    to:String,
}

#[derive(Serialize)]
struct ApplyResult {
    journal: Option<PathBuf>,
    moved: usize,
    failed: Vec<Failure>,
}

#[derive(Serialize)]
struct Report<'a> {
    mdoe: &'static str,
    plan: &'a Plan,
    result: Option<ApplyResult>,
}

fn cmd_scan(args: ScanArgs, force_apply: bool)-> Result<u8>{
    let (cfg, _) = Config::load(args.config.as_deref())?;
    let apply = force_apply || args.apply || cfg.general.always_apply;
    let min_size = args.min_size.unwrap_or(cfg.general.min_size_bytes);

    let plan = build_plan(&PlanInput {
        cfg: &cfg,
        sources: &args.paths,
        min_size,
    })?;

    let mut exit = EXIT_OK;
    let mut result = None;

    if apply && !plan.moves.is_empty() {
        let target_root = absolutize(&cfg.target_root());
        let outcome = executor::execute(&plan, cfg.general.collision, &target_root);

        let journal_path = if outcome.operations.is_empty() && outcome.failed.is_empty() {
            None
        } else {
            let j = journal::Journal::new(outcome.operations.clone(), outcome.failed.clone());
            Some(journal::write(&cfg.journal_dir(),&j)?)
        };
        if !outcome.failed.is_empty(){
            exit = EXIT_PARTIAL;
        }
        result = Some(ApplyResult {
            journal: journal_path,
            moved: outcome.operations.len(),
            failed:outcome.failed,
        });
    }

    if args.json {
        let report = Report {
            mode: if apply {"apply"} else {"dry-run"},
            plan: &plan,
            result,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(exit);
    }

    print_plan(&plan,apply);
    if let Some(res) = &result {
        println!(
            "\n{} {} file(s) moved.",
            "Applied:".green().bold(),
            res.moved
        );
        if let Some(j) = &res.journal {
            println!("Journal: {} (undo with `fo undo`)", j.display());
        }
        for f in &res.failed {
            eprintln!("{} {}: {}", "failed:".red(), f.from.display(),f.error);
        }
    } else if !apply && !plan.moves.is_empty() {
        println!(
            "\n{}",
            "Dry run: nothing was moved. Re-run with --apply to perform these moves.".yellow()
        );
    }
    Ok(exit)
}

fn print_plan(plan:&Plan,apply:bool){
    if plan.moves.is_empty(){
        println!("Nothing to move.");
    }else{
        let rows: Vec<Row>=plan
        .moves
        .iter()
        .map(|m| Row {
            rule:m.rule.clone(),
            size:human_size(m.size),
            from:m.from.display().to_string(),
            to: if m.replaces_existing{
                format!("{} (replaces existing)",m.to.display())
            }else{
                m.to.display().to_string()
            },
        })
        .collect();
    println!("{}", Table::new(rows).with(Style::rounded()));
    }
    println!(
        "{} planned, {} skipped, {} unclassified{}",
        plan.moves.len(),
        plan.skipped.len(),
        plan.unclassified.len(),
        if apply { "" } else { " (dry run)" }
    );
    for s in &plan.skipped {
        println!("  {} {} ({})", "skip".dimmed(), s.path.display(), s.reason);
    }
    for p in &plan.unclassified {
        println!("  {} {}", "unclassified".dimmed(), p.display());
    }
}

fn cmd_undo(journal_file: Option<PathBuf>, last: usize, config: Option<PathBuf>) -> Result<u8> {
    let targets: Vec<PathBUf> = match journal_file {
        Some(path) => vec![path],
        None => {
            let (cfg, _) = Config::load(config.as_deref())?;
            let dir = cfg.journal_dir();
            let mut all = journal::list(&dir)?;
            if all.is_empty() {
                return Err(FoError::NoJournal(dir).into());
            }
            let keep_from = all.len().saturating_sub(last.max(1));
            let mut chosen = all.split_off(keep_from);
            chosen.reverse(); // newest first
            chosen
        }
    };

    let mut exit = EXIT_OK;
    for path in targets {
        let report = journal::undo(&path)?;
        println!(
            "{} {} file(s) from {}",
            "Restored".green(),
            report.restored,
            path.display()
        );
        for f in &report.failed {
            eprintln!("{} {}: {}", "failed:".red(), f.from.display(),f.error);
            exit = EXIT_PARTIAL;
        }
    }
    Ok(exit)
}

fn cmd_config(path_only: bool, config: Option<PathBuf>)->Result<u8>{
    let path = match config {
        Some(p) => p,
        None => config::default_config_path()?,
    };
    if path_only {
        println!("{}", path.display());
        return Ok(EXIT_OK);
    }
    println!("# {}", path.display());
    let text = std::fs::read_to_string(&path).map_err(|_| FoError::ConfigNotFound(path.clone()))?;
    print!("{text}");
    Ok(EXIT_OK)
}
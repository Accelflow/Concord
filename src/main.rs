use clap::{Parser, Subcommand};
use concord::{engine::Engine, runner};
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

#[derive(Parser)]
#[command(
    name = "concord",
    version,
    about = "Local contract and property-driven test planning"
)]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Check definitions and their configuration references without collecting tests.
    Check {
        file: Option<std::path::PathBuf>,
        /// Check command references using this TOML file (defaults to concord.toml when no YAML path is given).
        #[arg(long)]
        config: Option<std::path::PathBuf>,
    },
    Scan,
    Plan {
        #[arg(long, default_value = "main")]
        base: String,
        #[arg(long)]
        all: bool,
    },
    Verify {
        #[arg(long, default_value = "main")]
        base: String,
        #[arg(long)]
        all: bool,
        /// Execute only if the recomputed plan has this ID.
        #[arg(long)]
        expect_plan: Option<String>,
    },
    Explain {
        test_id: String,
    },
}
fn execute(cli: &Cli) -> anyhow::Result<(Value, i32)> {
    if let Commands::Check { file, config } = &cli.command {
        let path = file
            .clone()
            .unwrap_or_else(concord::definition_source::default_check_path);
        let default_config = std::path::Path::new("concord.toml");
        let config = config.as_deref().or_else(|| {
            (file.is_none() && default_config.symlink_metadata().is_ok()).then_some(default_config)
        });
        let report = concord::definition_check::check(&path, config);
        let code = if report["valid"] == true { 0 } else { 2 };
        return Ok((report, code));
    }
    let mut e = Engine::open()?;
    match &cli.command {
        Commands::Check { .. } => unreachable!("definition check is handled without Engine"),
        Commands::Scan => Ok((serde_json::to_value(e.scan()?)?, 0)),
        Commands::Plan { base, all } => Ok((serde_json::to_value(e.plan(base, *all)?.0)?, 0)),
        Commands::Verify {
            base,
            all,
            expect_plan,
        } => {
            let run = e.verify(base, *all, expect_plan.as_deref())?;
            let code = run.exit_code;
            Ok((serde_json::to_value(run)?, code))
        }
        Commands::Explain { test_id } => Ok((e.explain(test_id)?, 0)),
    }
}
fn main() {
    let cli = Cli::parse();
    if let Err(error) = runner::install_interrupt_handler() {
        eprintln!("{error}");
        std::process::exit(2)
    }
    let (value, code) = match execute(&cli) {
        Ok(v) => v,
        Err(e) => {
            let code = if runner::INTERRUPTED.load(Ordering::SeqCst) {
                3
            } else {
                2
            };
            (
                json!({"schema_version":3,"status":"error","error":format!("{e:#}"),"exit_code":code}),
                code,
            )
        }
    };
    // JSON is the canonical result; the default prints the same structure indented.
    let output = if cli.json {
        serde_json::to_string(&value)
    } else {
        serde_json::to_string_pretty(&value)
    };
    match output {
        Ok(text) => println!("{text}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2)
        }
    }
    std::process::exit(code)
}

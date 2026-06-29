use std::path::PathBuf;

use chrono::NaiveDate;
use clap::Parser;
use clap::Subcommand;
use renogymon_archiver::archiver::ExportConfig;
use renogymon_archiver::archiver::run_export;
use renogymon_archiver::archiver::run_status;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "renogymon-archiver")]
#[command(about = "Export Renogy BMS metrics from VictoriaMetrics to Parquet")]
struct Args {
    /// VictoriaMetrics base URL
    #[arg(long, default_value = "http://localhost:8428")]
    vm_addr: String,

    /// Local staging directory for Parquet files
    #[arg(long, default_value = "/var/lib/renogymon-archiver/staging")]
    staging_dir: PathBuf,

    /// State file path
    #[arg(long, default_value = "/var/lib/renogymon-archiver/state.json")]
    state_file: PathBuf,

    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Export unarchived days from VM to the local Parquet staging dir
    Export {
        /// First-run backfill lower bound (YYYY-MM-DD); ignored once state exists
        #[arg(long)]
        start_date: Option<NaiveDate>,

        /// Export at most N days this run (bounds staging for large backfills)
        #[arg(long)]
        max_days: Option<usize>,
    },
    /// Show last exported day and staged files
    Status,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let default_filter = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter)),
        )
        .init();

    match args.command {
        Command::Export {
            start_date,
            max_days,
        } => {
            let cfg = ExportConfig {
                vm_addr: args.vm_addr,
                staging_dir: args.staging_dir,
                state_file: args.state_file,
                start_date,
                max_days,
            };
            run_export(&cfg).await?;
        }
        Command::Status => {
            run_status(&args.staging_dir, &args.state_file)?;
        }
    }
    Ok(())
}

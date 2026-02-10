use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

/// uniq — Research-driven AI technique discovery and implementation engine.
///
/// Searches academic literature for novel techniques relevant to your project,
/// extracts methodologies from papers, generates multiple variant implementations,
/// benchmarks them, and lets you merge approaches.
#[derive(Parser, Debug)]
#[command(name = "uniq", version, about)]
struct Cli {
    /// Path to the project to analyze (can also be set in the TUI).
    #[arg(short, long)]
    project: Option<String>,

    /// Description of what AI capability to add (can also be set in the TUI).
    #[arg(short, long)]
    description: Option<String>,

    /// Path to the sidecar directory (defaults to ./sidecar relative to the binary).
    #[arg(long)]
    sidecar_dir: Option<String>,

    /// Increase logging verbosity (-v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up logging.
    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    // Log to a file to avoid corrupting the TUI output. If the log file
    // can't be opened, silently discard logs rather than polluting the
    // alternate screen buffer.
    let log_dir = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("uniq");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("uniq.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path);

    match log_file {
        Ok(file) => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
                )
                .with_writer(std::sync::Mutex::new(file))
                .with_ansi(false)
                .init();
        }
        Err(_) => {
            // Fallback: discard all logs to avoid TUI corruption.
            tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::new("off"))
                .with_writer(std::io::sink)
                .init();
        }
    }

    // Load config.
    let _config = uniq_core::UniqConfig::load().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config: {}. Using defaults.", e);
        uniq_core::UniqConfig::default()
    });

    tracing::info!("Starting uniq v{}", env!("CARGO_PKG_VERSION"));

    // Determine sidecar directory.
    let sidecar_dir = if let Some(ref dir) = cli.sidecar_dir {
        std::path::PathBuf::from(dir)
    } else {
        // Default: look for ./sidecar relative to current directory.
        std::env::current_dir()?.join("sidecar")
    };

    // Start the TUI.
    let mut app = uniq_tui::App::new(sidecar_dir);

    // Pre-fill project info from CLI args if provided.
    if let Some(ref project) = cli.project {
        app.set_initial_project(project.clone());
    }
    if let Some(ref description) = cli.description {
        app.set_initial_description(description.clone());
    }

    app.run().await?;

    tracing::info!("uniq exited cleanly");
    Ok(())
}

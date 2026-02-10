//! Manages the lifecycle of the Python sidecar process.
//!
//! The sidecar is a FastAPI server that runs on localhost. This module handles:
//! - Spawning the process via `uv run`
//! - Waiting for it to become healthy
//! - Graceful and forced shutdown

use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

/// How long to wait for the sidecar to report healthy (in seconds).
const STARTUP_TIMEOUT_SECS: u64 = 60;

/// How long between health check polls during startup (in milliseconds).
const HEALTH_POLL_INTERVAL_MS: u64 = 250;

/// Manages a running Python sidecar process.
pub struct SidecarManager {
    child: Option<Child>,
    port: u16,
    sidecar_dir: PathBuf,
}

impl SidecarManager {
    /// Create a new manager. Does not start the sidecar yet.
    pub fn new(sidecar_dir: PathBuf) -> Self {
        Self {
            child: None,
            port: 0,
            sidecar_dir,
        }
    }

    /// Get the port the sidecar is running on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Get the base URL for the sidecar API.
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Whether the sidecar process is currently running.
    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    /// Start the sidecar process and wait for it to become healthy.
    pub async fn start(&mut self) -> anyhow::Result<()> {
        if self.child.is_some() {
            anyhow::bail!("Sidecar is already running");
        }

        // Find a free port.
        let port = find_free_port()?;
        self.port = port;

        info!(port = port, dir = %self.sidecar_dir.display(), "Starting Python sidecar");

        // Spawn the process using `uv run`.
        // We set the working directory to the sidecar project so that
        // `python -m src.server` resolves `src` as a local package.
        let mut child = Command::new("uv")
            .args([
                "run",
                "--project",
                self.sidecar_dir.to_str().unwrap_or("."),
                "python",
                "-m",
                "src.server",
                "--port",
                &port.to_string(),
            ])
            .current_dir(&self.sidecar_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        // Spawn a task to log stderr output.
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!(target: "sidecar::stderr", "{}", line);
                }
            });
        }

        // Spawn a task to log stdout output.
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    debug!(target: "sidecar::stdout", "{}", line);
                }
            });
        }

        self.child = Some(child);

        // Wait for the sidecar to become healthy.
        self.wait_for_healthy().await?;

        info!(port = port, "Python sidecar is ready");
        Ok(())
    }

    /// Poll the health endpoint until the sidecar reports healthy.
    async fn wait_for_healthy(&self) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/health", self.base_url());
        let deadline =
            tokio::time::Instant::now() + tokio::time::Duration::from_secs(STARTUP_TIMEOUT_SECS);

        loop {
            if tokio::time::Instant::now() > deadline {
                anyhow::bail!(
                    "Sidecar failed to start within {} seconds. \
                     Make sure Python and uv are installed and the sidecar dependencies are available.",
                    STARTUP_TIMEOUT_SECS
                );
            }

            match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    return Ok(());
                }
                Ok(resp) => {
                    debug!(status = %resp.status(), "Sidecar not ready yet");
                }
                Err(_) => {
                    // Connection refused — sidecar isn't listening yet.
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(HEALTH_POLL_INTERVAL_MS)).await;
        }
    }

    /// Gracefully shut down the sidecar.
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        // Compute URL before taking the mutable borrow on self.child.
        let shutdown_url = format!("{}/api/shutdown", self.base_url());

        if let Some(ref mut child) = self.child {
            info!("Shutting down Python sidecar");

            // Try graceful shutdown first.
            let client = reqwest::Client::new();
            let url = shutdown_url;
            match client.post(&url).send().await {
                Ok(_) => {
                    debug!("Sent shutdown request to sidecar");
                    // Give it a moment to exit cleanly.
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
                Err(e) => {
                    warn!("Failed to send shutdown request: {}", e);
                }
            }

            // Force kill if still running.
            match child.try_wait() {
                Ok(Some(_status)) => {
                    debug!("Sidecar exited cleanly");
                }
                Ok(None) => {
                    warn!("Sidecar still running, sending kill signal");
                    if let Err(e) = child.kill().await {
                        error!("Failed to kill sidecar: {}", e);
                    }
                }
                Err(e) => {
                    error!("Error checking sidecar status: {}", e);
                }
            }

            self.child = None;
        }

        Ok(())
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        // Best-effort synchronous cleanup.
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
    }
}

/// Find an available TCP port on localhost.
fn find_free_port() -> anyhow::Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

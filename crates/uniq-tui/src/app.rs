//! Main application state and render loop.

use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Tabs;
use ratatui::Terminal;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use uniq_sidecar::{SidecarClient, SidecarManager};

use crate::action::{Action, InputMode, Phase};
use crate::components::benchmark_dashboard::BenchmarkDashboardComponent;
use crate::components::help::HelpComponent;
use crate::components::merge_dialog::MergeDialogComponent;
use crate::components::project_intake::ProjectIntakeComponent;
use crate::components::research_explorer::ResearchExplorerComponent;
use crate::components::status_bar::StatusBarComponent;
use crate::components::technique_cards::TechniqueCardsComponent;
use crate::components::variant_builder::VariantBuilderComponent;
use crate::components::Component;
use crate::event::{self, EventHandler, InputModeFlag};
use crate::theme::Theme;

/// Main application state.
pub struct App {
    /// Current active phase.
    current_phase: Phase,
    /// Whether the app should exit.
    should_quit: bool,
    /// Shared flag to tell the EventHandler which key-mapping to use.
    input_mode_flag: InputModeFlag,

    // ── Sidecar ──────────────────────────────────────────────
    /// Path to the Python sidecar directory.
    sidecar_dir: PathBuf,
    /// Sidecar process manager (owns the child process).
    sidecar_manager: Option<SidecarManager>,
    /// HTTP client for sidecar API calls (shared across async tasks).
    sidecar_client: Option<Arc<SidecarClient>>,
    /// Receiver for the background sidecar startup result.
    sidecar_startup_rx: Option<
        tokio::sync::oneshot::Receiver<Result<(SidecarManager, Arc<SidecarClient>), String>>,
    >,

    // ── Shared state for async operations ────────────────────
    /// The user's description (saved after project submission).
    user_description: String,

    // Components
    project_intake: ProjectIntakeComponent,
    research_explorer: ResearchExplorerComponent,
    technique_cards: TechniqueCardsComponent,
    variant_builder: VariantBuilderComponent,
    benchmark_dashboard: BenchmarkDashboardComponent,
    merge_dialog: MergeDialogComponent,
    status_bar: StatusBarComponent,
    help: HelpComponent,
}

impl App {
    pub fn new(sidecar_dir: PathBuf) -> Self {
        Self {
            current_phase: Phase::ProjectIntake,
            should_quit: false,
            input_mode_flag: event::new_input_mode_flag(),
            sidecar_dir,
            sidecar_manager: None,
            sidecar_client: None,
            sidecar_startup_rx: None,
            user_description: String::new(),
            project_intake: ProjectIntakeComponent::new(),
            research_explorer: ResearchExplorerComponent::new(),
            technique_cards: TechniqueCardsComponent::new(),
            variant_builder: VariantBuilderComponent::new(),
            benchmark_dashboard: BenchmarkDashboardComponent::new(),
            merge_dialog: MergeDialogComponent::new(),
            status_bar: StatusBarComponent::new(),
            help: HelpComponent::new(),
        }
    }

    /// Pre-fill the project path from CLI args.
    pub fn set_initial_project(&mut self, path: String) {
        self.project_intake.path_input = path.clone();
        self.project_intake.cursor = path.len();
    }

    /// Pre-fill the description from CLI args.
    pub fn set_initial_description(&mut self, description: String) {
        self.project_intake.description_input = description;
    }

    /// Run the TUI application.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        // Set up terminal.
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Create the action channel.
        let (tx, mut rx) = mpsc::unbounded_channel::<Action>();

        // Start the event handler with the shared input mode flag.
        let event_tx = tx.clone();
        let mode_flag = self.input_mode_flag.clone();
        let event_handler = EventHandler::new(event_tx, Duration::from_millis(100), mode_flag);
        tokio::spawn(async move {
            event_handler.run().await;
        });

        // Start the Python sidecar in the background so the TUI renders immediately.
        self.start_sidecar_async(tx.clone());

        // Set initial input mode (Phase 1 starts in editing mode).
        self.sync_input_mode();

        // Main loop.
        loop {
            // Render.
            terminal.draw(|frame| {
                self.render(frame);
            })?;

            // Check if the background sidecar startup has completed.
            self.poll_sidecar_startup();

            // Process actions.
            if let Some(action) = rx.recv().await {
                self.handle_action(&action, &tx);

                if self.should_quit {
                    break;
                }
            }
        }

        // Shut down the sidecar.
        self.shutdown_sidecar().await;

        // Restore terminal.
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste
        )?;
        terminal.show_cursor()?;

        Ok(())
    }

    /// Spawn sidecar startup in the background. The TUI renders immediately
    /// while the sidecar boots. A SidecarReady/SidecarFailed action is sent
    /// when it completes.
    fn start_sidecar_async(&mut self, tx: mpsc::UnboundedSender<Action>) {
        let sidecar_dir = self.sidecar_dir.clone();
        info!(dir = %sidecar_dir.display(), "Starting sidecar (background)");
        let _ = tx.send(Action::SetStatus("Starting Python sidecar...".to_string()));

        // Use a oneshot to send the manager + client back to the main task.
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let mut manager = SidecarManager::new(sidecar_dir);
            match manager.start().await {
                Ok(()) => {
                    let base_url = manager.base_url();
                    info!(url = %base_url, "Sidecar started successfully");
                    let client = Arc::new(SidecarClient::new(base_url));
                    let _ = result_tx.send(Ok((manager, client)));
                    let _ = tx.send(Action::SetStatus("Sidecar ready".to_string()));
                }
                Err(e) => {
                    error!("Failed to start sidecar: {}", e);
                    let _ = result_tx.send(Err(format!("{}", e)));
                    let _ = tx.send(Action::SetStatus(format!("Sidecar failed: {}", e)));
                }
            }
        });

        // Store the receiver so we can poll it from the main loop.
        self.sidecar_startup_rx = Some(result_rx);
    }

    /// Non-blocking check whether the background sidecar startup has completed.
    fn poll_sidecar_startup(&mut self) {
        if let Some(ref mut rx) = self.sidecar_startup_rx {
            match rx.try_recv() {
                Ok(Ok((manager, client))) => {
                    self.sidecar_manager = Some(manager);
                    self.sidecar_client = Some(client);
                    self.sidecar_startup_rx = None;
                    info!("Sidecar startup received in main loop");
                }
                Ok(Err(e)) => {
                    warn!("Sidecar startup failed: {}", e);
                    self.sidecar_startup_rx = None;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Still starting — do nothing.
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    // Sender was dropped (task panicked?).
                    warn!("Sidecar startup task dropped unexpectedly");
                    self.sidecar_startup_rx = None;
                }
            }
        }
    }

    /// Shut down the sidecar process gracefully.
    async fn shutdown_sidecar(&mut self) {
        if let Some(ref mut manager) = self.sidecar_manager {
            info!("Shutting down sidecar");
            if let Err(e) = manager.shutdown().await {
                warn!("Sidecar shutdown error: {}", e);
            }
        }
    }

    /// Determine and set the correct input mode based on the current phase
    /// and component state. Called after every action.
    fn sync_input_mode(&self) {
        let mode = self.current_input_mode();
        event::set_input_mode(&self.input_mode_flag, mode);
    }

    /// What input mode should be active right now?
    fn current_input_mode(&self) -> InputMode {
        // If help or merge dialog is visible, stay in normal mode
        // so Esc and other keys work as expected.
        if self.help.visible || self.merge_dialog.visible {
            return InputMode::Normal;
        }

        match self.current_phase {
            Phase::ProjectIntake => {
                if self.project_intake.wants_input() {
                    InputMode::Editing
                } else {
                    InputMode::Normal
                }
            }
            _ => InputMode::Normal,
        }
    }

    /// Dispatch an action to all relevant components.
    fn handle_action(&mut self, action: &Action, tx: &mpsc::UnboundedSender<Action>) {
        // Global actions first.
        match action {
            Action::Quit => {
                self.should_quit = true;
                return;
            }
            Action::GoToPhase(phase) => {
                self.current_phase = *phase;
                self.status_bar.current_phase = *phase;
                self.auto_trigger_phase(*phase, tx);
            }
            Action::NextPhase => {
                // In editing mode, NextPhase is not sent (Tab → SwitchInputField),
                // but if it arrives anyway (e.g. Right arrow in editing mode), ignore it.
                if self.current_input_mode() != InputMode::Editing {
                    if let Some(next) = self.current_phase.next() {
                        if !self.merge_dialog.visible {
                            self.current_phase = next;
                            self.status_bar.current_phase = next;
                            self.auto_trigger_phase(next, tx);
                        }
                    }
                }
            }
            Action::PrevPhase => {
                if self.current_input_mode() != InputMode::Editing {
                    if let Some(prev) = self.current_phase.prev() {
                        if !self.merge_dialog.visible {
                            self.current_phase = prev;
                            self.status_bar.current_phase = prev;
                        }
                    }
                }
            }
            // ── Async operations triggered by components ─────────
            Action::SubmitProject { path, description } => {
                self.user_description = description.clone();
                self.spawn_analyze_project(path.clone(), description.clone(), tx.clone());
            }
            Action::StartResearch => {
                if !self.research_explorer.searching {
                    self.research_explorer.searching = true;
                    self.spawn_search_papers(tx.clone());
                }
            }
            Action::StartExtraction(papers) => {
                if !self.technique_cards.extracting {
                    self.technique_cards.extracting = true;
                    self.technique_cards.extraction_attempted = true;
                    self.technique_cards.progress = (0, papers.len());
                    self.spawn_extract_techniques(papers.clone(), tx.clone());
                }
            }
            Action::StartGeneration => {
                if !self.variant_builder.generating {
                    self.spawn_generate_variants(tx.clone());
                }
            }
            Action::StartBenchmark => {
                if !self.benchmark_dashboard.benchmarking {
                    self.spawn_run_benchmarks(tx.clone());
                }
            }
            _ => {}
        }

        // Forward to the active phase component.
        let result = match self.current_phase {
            Phase::ProjectIntake => self.project_intake.handle_action(action),
            Phase::ResearchDiscovery => self.research_explorer.handle_action(action),
            Phase::TechniqueSelection => self.technique_cards.handle_action(action),
            Phase::VariantGeneration => self.variant_builder.handle_action(action),
            Phase::Benchmarking => self.benchmark_dashboard.handle_action(action),
        };

        // Always forward to overlays and status bar.
        self.merge_dialog.handle_action(action);
        self.help.handle_action(action);
        self.status_bar.handle_action(action);

        // Sync input mode after every action (phase may have changed,
        // or the component state may have changed).
        self.sync_input_mode();

        // Auto-advance to Phase 2 after project analysis completes.
        if matches!(action, Action::ProjectAnalyzed(_)) && self.project_intake.profile.is_some() {
            self.current_phase = Phase::ResearchDiscovery;
            self.status_bar.current_phase = Phase::ResearchDiscovery;
            self.sync_input_mode();
            self.auto_trigger_phase(Phase::ResearchDiscovery, tx);
        }

        // Check if all technique extraction is complete.
        if self.technique_cards.extracting {
            if matches!(
                action,
                Action::TechniqueExtracted(_) | Action::TechniqueExtractionFailed { .. }
            ) {
                let (done, total) = self.technique_cards.progress;
                if total > 0 && done >= total {
                    self.handle_action(&Action::ExtractionComplete, tx);
                }
            }
        }

        // Check if all variant generation is complete.
        if self.variant_builder.generating {
            if matches!(
                action,
                Action::VariantGenerated(_) | Action::VariantGenerationFailed { .. }
            ) {
                let all_done = self.variant_builder.variants.iter().all(|v| {
                    !matches!(
                        v.status,
                        uniq_core::variant::VariantStatus::Pending
                            | uniq_core::variant::VariantStatus::Generating
                    )
                });
                if all_done {
                    self.handle_action(&Action::GenerationComplete, tx);
                }
            }
        }

        // Handle chained actions from components.
        if let Some(chained) = result {
            self.handle_action(&chained, tx);
        }
    }

    // ── Async task spawners ─────────────────────────────────────

    /// Spawn a task to analyze the project via the sidecar.
    fn spawn_analyze_project(
        &self,
        path: String,
        description: String,
        tx: mpsc::UnboundedSender<Action>,
    ) {
        let Some(client) = self.sidecar_client.clone() else {
            let _ = tx.send(Action::ProjectAnalysisFailed(
                "Sidecar is not running. Cannot analyze project.".to_string(),
            ));
            return;
        };

        let _ = tx.send(Action::SetStatus("Analyzing project...".to_string()));

        tokio::spawn(async move {
            match client
                .analyze_project(PathBuf::from(&path), description)
                .await
            {
                Ok(profile) => {
                    info!("Project analyzed: {} files", profile.file_count);
                    let _ = tx.send(Action::ProjectAnalyzed(Box::new(profile)));
                    let _ = tx.send(Action::SetStatus(
                        "Project analyzed successfully".to_string(),
                    ));
                }
                Err(e) => {
                    error!("Project analysis failed: {}", e);
                    let _ = tx.send(Action::ProjectAnalysisFailed(format!("{}", e)));
                    let _ = tx.send(Action::SetStatus(format!("Analysis failed: {}", e)));
                }
            }
        });
    }

    /// Spawn a task to search for academic papers via the sidecar.
    fn spawn_search_papers(&self, tx: mpsc::UnboundedSender<Action>) {
        let Some(client) = self.sidecar_client.clone() else {
            let _ = tx.send(Action::ResearchFailed(
                "Sidecar is not running.".to_string(),
            ));
            return;
        };

        // Build search queries from the project profile and user description.
        let description = self.user_description.clone();
        let summary = self
            .project_intake
            .profile
            .as_ref()
            .map(|p| p.summary.clone())
            .unwrap_or_default();

        let _ = tx.send(Action::SetStatus("Searching for papers...".to_string()));

        tokio::spawn(async move {
            // Generate diverse search queries from the user's description
            // and the project summary. API search endpoints have query length
            // limits, so we extract short, focused phrases rather than
            // sending the full (potentially very long) description.
            let short_desc = if description.len() > 120 {
                let truncated = &description[..120];
                match truncated.rfind(' ') {
                    Some(pos) => truncated[..pos].to_string(),
                    None => truncated.to_string(),
                }
            } else {
                description.clone()
            };

            let short_summary = if summary.len() > 120 {
                let truncated = &summary[..120];
                match truncated.rfind(' ') {
                    Some(pos) => truncated[..pos].to_string(),
                    None => truncated.to_string(),
                }
            } else {
                summary.clone()
            };

            let queries = vec![
                short_desc.clone(),
                format!("{} machine learning", short_desc),
                format!("{} deep learning", short_desc),
                short_summary,
            ];

            let total_queries = queries.len();
            let mut had_error = false;

            // Send queries one at a time so we can show per-query progress.
            for (i, query) in queries.iter().enumerate() {
                let _ = tx.send(Action::SearchQueryStarted {
                    query: query.clone(),
                    query_idx: i,
                    total_queries,
                });

                match client
                    .search_papers(vec![query.clone()], 20, Some(2020), None, true)
                    .await
                {
                    Ok(papers) => {
                        if !papers.is_empty() {
                            info!(
                                "Query {}/{}: found {} papers",
                                i + 1,
                                total_queries,
                                papers.len()
                            );
                            let _ = tx.send(Action::PapersFound(papers));
                        }
                    }
                    Err(e) => {
                        warn!("Query {}/{} failed: {}", i + 1, total_queries, e);
                        had_error = true;
                        // Continue with remaining queries rather than aborting.
                    }
                }
            }

            if had_error {
                let _ = tx.send(Action::ResearchComplete);
            } else {
                let _ = tx.send(Action::ResearchComplete);
            }
        });
    }

    /// Spawn tasks to extract techniques from selected papers.
    fn spawn_extract_techniques(
        &self,
        papers: Vec<uniq_core::research::PaperMeta>,
        tx: mpsc::UnboundedSender<Action>,
    ) {
        let Some(client) = self.sidecar_client.clone() else {
            let _ = tx.send(Action::TechniqueExtractionFailed {
                paper_id: "all".to_string(),
                error: "Sidecar is not running.".to_string(),
            });
            return;
        };

        let user_request = self.user_description.clone();
        let project_summary = self
            .project_intake
            .profile
            .as_ref()
            .map(|p| p.summary.clone())
            .unwrap_or_default();

        let total = papers.len();
        let _ = tx.send(Action::SetStatus(format!(
            "Extracting techniques from {} papers...",
            total
        )));

        // Spawn one task per paper (they run concurrently).
        for paper in papers {
            let client = client.clone();
            let tx = tx.clone();
            let user_request = user_request.clone();
            let project_summary = project_summary.clone();

            // Skip papers that have neither a PDF URL nor a DOI — we can't download anything.
            if paper.pdf_url.is_none() && paper.doi.is_none() {
                let _ = tx.send(Action::TechniqueExtractionFailed {
                    paper_id: paper.id.clone(),
                    error: "No PDF URL or DOI available".to_string(),
                });
                continue;
            }

            tokio::spawn(async move {
                // Notify UI that this paper is being processed.
                let _ = tx.send(Action::ExtractionStarted {
                    paper_title: paper.title.clone(),
                });

                match client
                    .extract_technique(
                        paper.pdf_url.clone(),
                        paper.id.clone(),
                        paper.title.clone(),
                        project_summary,
                        user_request,
                        paper.doi.clone(),
                    )
                    .await
                {
                    Ok(technique) => {
                        info!("Extracted technique: {}", technique.name);
                        let _ = tx.send(Action::TechniqueExtracted(Box::new(technique)));
                    }
                    Err(e) => {
                        warn!("Extraction failed for {}: {}", paper.id, e);
                        let _ = tx.send(Action::TechniqueExtractionFailed {
                            paper_id: paper.id.clone(),
                            error: format!("{}", e),
                        });
                    }
                }
            });
        }
    }

    /// Automatically trigger async operations when entering a new phase.
    /// This prevents the user from having to manually start each phase.
    fn auto_trigger_phase(&self, phase: Phase, tx: &mpsc::UnboundedSender<Action>) {
        match phase {
            Phase::ResearchDiscovery => {
                // Auto-start research if project is analyzed and no papers yet.
                if self.project_intake.profile.is_some()
                    && self.research_explorer.papers.is_empty()
                    && !self.research_explorer.searching
                {
                    let _ = tx.send(Action::StartResearch);
                }
            }
            Phase::TechniqueSelection => {
                // Auto-start extraction if papers exist but no techniques yet
                // and extraction hasn't been attempted before.
                if !self.research_explorer.papers.is_empty()
                    && self.technique_cards.techniques.is_empty()
                    && !self.technique_cards.extracting
                    && !self.technique_cards.extraction_attempted
                {
                    let papers = self.research_explorer.papers.clone();
                    let _ = tx.send(Action::StartExtraction(papers));
                }
            }
            Phase::VariantGeneration => {
                // Auto-start generation if techniques are selected but no variants yet.
                if self.technique_cards.selected_count() > 0
                    && self.variant_builder.variants.is_empty()
                    && !self.variant_builder.generating
                {
                    let _ = tx.send(Action::StartGeneration);
                }
            }
            Phase::Benchmarking => {
                // Auto-start benchmarks if variants are ready but none benchmarked.
                let has_ready = self
                    .variant_builder
                    .variants
                    .iter()
                    .any(|v| v.status == uniq_core::variant::VariantStatus::Ready);
                let none_benchmarked = self.benchmark_dashboard.variants.is_empty()
                    || self
                        .benchmark_dashboard
                        .variants
                        .iter()
                        .all(|v| v.benchmark_results.is_none());

                if has_ready && none_benchmarked && !self.benchmark_dashboard.benchmarking {
                    let _ = tx.send(Action::StartBenchmark);
                }
            }
            _ => {}
        }
    }

    /// Spawn tasks to generate variants for all selected techniques.
    fn spawn_generate_variants(&mut self, tx: mpsc::UnboundedSender<Action>) {
        let Some(client) = self.sidecar_client.clone() else {
            let _ = tx.send(Action::VariantGenerationFailed {
                variant_id: "all".to_string(),
                error: "Sidecar is not running.".to_string(),
            });
            return;
        };

        let profile = match self.project_intake.profile.clone() {
            Some(p) => p,
            None => {
                let _ = tx.send(Action::SetStatus(
                    "No project profile — analyze a project first.".to_string(),
                ));
                return;
            }
        };

        // Collect selected techniques and create variant stubs.
        let selected_techniques: Vec<_> = self
            .technique_cards
            .techniques
            .iter()
            .filter(|t| t.selected)
            .cloned()
            .collect();

        if selected_techniques.is_empty() {
            let _ = tx.send(Action::SetStatus(
                "No techniques selected. Go back to Phase 3 and select some.".to_string(),
            ));
            return;
        }

        self.variant_builder.generating = true;

        let total = selected_techniques.len();
        let _ = tx.send(Action::SetStatus(format!(
            "Generating {} variants...",
            total
        )));

        // Create Variant stubs and spawn generation tasks.
        for (i, technique) in selected_techniques.into_iter().enumerate() {
            let index = i + 1;
            let variant = uniq_core::variant::Variant::from_technique(index, technique.clone());
            let branch_name = variant.branch_name.clone();
            let variant_id = variant.id.0.clone();

            // Add the pending variant to the builder so the UI shows it immediately.
            self.variant_builder.variants.push(variant);

            let client = client.clone();
            let tx = tx.clone();
            let profile = profile.clone();
            let technique_for_result = technique.clone();

            tokio::spawn(async move {
                match client
                    .generate_variant(technique, profile, branch_name.clone())
                    .await
                {
                    Ok(result) => {
                        if result.success {
                            info!(
                                "Variant {} generated: {} files modified",
                                variant_id,
                                result.modified_files.len()
                            );
                            let mut v = uniq_core::variant::Variant::from_technique(
                                index,
                                technique_for_result,
                            );
                            v.status = uniq_core::variant::VariantStatus::Ready;
                            v.modified_files = result.modified_files;
                            v.new_dependencies = result.new_dependencies;
                            let _ = tx.send(Action::VariantGenerated(Box::new(v)));
                        } else {
                            let err = result.error.unwrap_or_else(|| "Unknown error".to_string());
                            let _ = tx.send(Action::VariantGenerationFailed {
                                variant_id,
                                error: err,
                            });
                        }
                    }
                    Err(e) => {
                        error!("Variant generation failed for {}: {}", variant_id, e);
                        let _ = tx.send(Action::VariantGenerationFailed {
                            variant_id,
                            error: format!("{}", e),
                        });
                    }
                }
            });
        }

        // Generation completion is detected in handle_action by checking
        // whether all variants have left the Pending/Generating state.
    }

    /// Spawn tasks to run benchmarks on all ready variants.
    fn spawn_run_benchmarks(&mut self, tx: mpsc::UnboundedSender<Action>) {
        let Some(client) = self.sidecar_client.clone() else {
            let _ = tx.send(Action::SetStatus("Sidecar is not running.".to_string()));
            return;
        };

        let project_path = self.project_intake.profile.as_ref().map(|p| p.path.clone());
        let project_path = match project_path {
            Some(p) => PathBuf::from(p),
            None => {
                let _ = tx.send(Action::SetStatus("No project path available.".to_string()));
                return;
            }
        };

        // Sync variants into the benchmark dashboard.
        self.benchmark_dashboard.variants = self.variant_builder.variants.clone();
        self.benchmark_dashboard.benchmarking = true;

        let ready_branches: Vec<String> = self
            .variant_builder
            .variants
            .iter()
            .filter(|v| v.status == uniq_core::variant::VariantStatus::Ready)
            .map(|v| v.branch_name.clone())
            .collect();

        if ready_branches.is_empty() {
            let _ = tx.send(Action::SetStatus(
                "No ready variants to benchmark.".to_string(),
            ));
            self.benchmark_dashboard.benchmarking = false;
            return;
        }

        let user_request = self.user_description.clone();
        let _ = tx.send(Action::SetStatus(format!(
            "Running benchmarks on {} variants...",
            ready_branches.len()
        )));

        // Run execution benchmarks and LLM judge in parallel.
        let client_exec = client.clone();
        let client_judge = client;
        let branches_exec = ready_branches.clone();
        let branches_judge = ready_branches;
        let path_exec = project_path.clone();
        let path_judge = project_path;
        let tx_exec = tx.clone();
        let tx_judge = tx;

        // Execution benchmarks
        tokio::spawn(async move {
            match client_exec
                .run_benchmark(branches_exec, path_exec, vec![], 300)
                .await
            {
                Ok(results) => {
                    for (branch, metrics) in results {
                        info!("Benchmark for {}: build={}", branch, metrics.build_success);
                        let _ = tx_exec.send(Action::BenchmarkUpdated { variant_id: branch });
                    }
                }
                Err(e) => {
                    error!("Execution benchmark failed: {}", e);
                    let _ = tx_exec.send(Action::SetStatus(format!("Benchmark failed: {}", e)));
                }
            }
        });

        // LLM judge
        tokio::spawn(async move {
            match client_judge
                .llm_judge(branches_judge, path_judge, user_request)
                .await
            {
                Ok(scores) => {
                    for (branch, _judge_scores) in scores {
                        let _ = tx_judge.send(Action::BenchmarkUpdated { variant_id: branch });
                    }
                    let _ = tx_judge.send(Action::BenchmarkComplete);
                }
                Err(e) => {
                    error!("LLM judge failed: {}", e);
                    let _ = tx_judge.send(Action::SetStatus(format!("LLM judge failed: {}", e)));
                    let _ = tx_judge.send(Action::BenchmarkComplete);
                }
            }
        });
    }

    /// Render the full UI.
    fn render(&self, frame: &mut ratatui::Frame) {
        let area = frame.area();

        let chunks = Layout::vertical([
            Constraint::Length(2), // Tab bar
            Constraint::Min(10),   // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(area);

        // Tab bar
        self.render_tabs(frame, chunks[0]);

        // Main content
        match self.current_phase {
            Phase::ProjectIntake => self.project_intake.render(frame, chunks[1]),
            Phase::ResearchDiscovery => self.research_explorer.render(frame, chunks[1]),
            Phase::TechniqueSelection => self.technique_cards.render(frame, chunks[1]),
            Phase::VariantGeneration => self.variant_builder.render(frame, chunks[1]),
            Phase::Benchmarking => self.benchmark_dashboard.render(frame, chunks[1]),
        }

        // Status bar
        self.status_bar.render(frame, chunks[2]);

        // Overlays (rendered on top)
        self.merge_dialog.render(frame, area);
        self.help.render(frame, area);
    }

    /// Render the phase tab bar.
    fn render_tabs(&self, frame: &mut ratatui::Frame, area: Rect) {
        let titles: Vec<Line> = Phase::all()
            .iter()
            .map(|phase| {
                let style = if *phase == self.current_phase {
                    Theme::tab_active()
                } else {
                    Theme::tab_inactive()
                };
                Line::from(Span::styled(phase.label(), style))
            })
            .collect();

        let tabs = Tabs::new(titles)
            .select(self.current_phase.index())
            .divider(Span::styled(" | ", Theme::dim()))
            .highlight_style(Theme::tab_active());

        frame.render_widget(tabs, area);
    }
}

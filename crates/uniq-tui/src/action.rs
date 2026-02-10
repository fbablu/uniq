//! Action enum — the central message bus for the TUI.
//! All user interactions and async results flow through here.

use uniq_core::project::ProjectProfile;
use uniq_core::research::{PaperMeta, TechniqueCard};
use uniq_core::variant::Variant;

/// Every possible action that can occur in the application.
#[derive(Debug, Clone)]
pub enum Action {
    // ── Navigation ──────────────────────────────────────────
    /// Switch to a specific phase tab.
    GoToPhase(Phase),
    /// Move to the next phase.
    NextPhase,
    /// Move to the previous phase.
    PrevPhase,

    // ── Global ──────────────────────────────────────────────
    /// Quit the application.
    Quit,
    /// Toggle help overlay.
    ToggleHelp,
    /// Display a status message in the status bar.
    SetStatus(String),
    /// Clear the status message.
    ClearStatus,
    /// A tick event for animations and polling.
    Tick,

    // ── Phase 1: Project Intake ─────────────────────────────
    /// User submitted project path and description.
    SubmitProject {
        path: String,
        description: String,
    },
    /// Project analysis completed.
    ProjectAnalyzed(Box<ProjectProfile>),
    /// Project analysis failed.
    ProjectAnalysisFailed(String),

    // ── Phase 2: Research Discovery ─────────────────────────
    /// Start searching for papers.
    StartResearch,
    /// A search query is starting (for progress display).
    SearchQueryStarted {
        query: String,
        query_idx: usize,
        total_queries: usize,
    },
    /// Papers found (batch update).
    PapersFound(Vec<PaperMeta>),
    /// Research search completed.
    ResearchComplete,
    /// Research failed.
    ResearchFailed(String),

    // ── Phase 3: Technique Selection ────────────────────────
    /// Start extracting techniques from selected papers.
    StartExtraction(Vec<PaperMeta>),
    /// Extraction started for a specific paper (for progress display).
    ExtractionStarted {
        paper_title: String,
    },
    /// A technique card was extracted.
    TechniqueExtracted(Box<TechniqueCard>),
    /// Technique extraction failed for a paper.
    TechniqueExtractionFailed {
        paper_id: String,
        error: String,
    },
    /// All extraction complete.
    ExtractionComplete,
    /// Toggle selection of a technique.
    ToggleTechnique(usize),
    /// Confirm technique selection and proceed to generation.
    ConfirmTechniques,

    // ── Phase 4: Variant Generation ─────────────────────────
    /// Start generating variants.
    StartGeneration,
    /// A variant was generated successfully.
    VariantGenerated(Box<Variant>),
    /// A variant generation failed.
    VariantGenerationFailed {
        variant_id: String,
        error: String,
    },
    /// All variants generated.
    GenerationComplete,

    // ── Phase 5: Benchmarking ───────────────────────────────
    /// Start benchmarking all variants.
    StartBenchmark,
    /// Benchmark results updated for a variant.
    BenchmarkUpdated {
        variant_id: String,
    },
    /// All benchmarks complete.
    BenchmarkComplete,
    /// User rated a variant.
    UserRated {
        variant_id: String,
        stars: u8,
        notes: String,
    },

    // ── Merging ─────────────────────────────────────────────
    /// Open the merge dialog.
    OpenMergeDialog,
    /// Close the merge dialog.
    CloseMergeDialog,
    /// Start merging two variants.
    StartMerge {
        variant_a_id: String,
        variant_b_id: String,
        blend_a: u8,
        blend_b: u8,
    },
    /// Merge completed.
    MergeComplete(Box<Variant>),
    /// Merge failed.
    MergeFailed(String),

    // ── Text Input ───────────────────────────────────────────
    /// A character was typed (only sent when in input mode).
    CharInput(char),
    /// Backspace pressed (only sent when in input mode).
    BackspaceInput,
    /// Delete word (Ctrl+Backspace or Ctrl+W).
    DeleteWord,
    /// Insert a newline in the current text field (Enter in multi-line fields).
    NewlineInput,
    /// Switch focus between input fields (Tab in input mode).
    SwitchInputField,
    /// Submit the form (Ctrl+Enter in editing mode).
    SubmitForm,
    /// Paste text from clipboard (Ctrl+V in editing mode).
    PasteInput,
    /// Bulk paste from bracketed paste mode (terminal sends entire text at once).
    PasteBulk(String),

    // ── Scrolling / Selection ───────────────────────────────
    ScrollUp,
    ScrollDown,
    SelectNext,
    SelectPrev,
    Confirm,
}

/// Whether the app is in a text-input mode where raw keys should
/// be forwarded to the active component instead of interpreted as
/// global shortcuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Normal mode — keys are global shortcuts.
    Normal,
    /// Text input mode — keys go to the focused text field.
    Editing,
}

/// The five pipeline phases, plus the merge view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    ProjectIntake,
    ResearchDiscovery,
    TechniqueSelection,
    VariantGeneration,
    Benchmarking,
}

impl Phase {
    /// Get all phases in order.
    pub fn all() -> &'static [Phase] {
        &[
            Phase::ProjectIntake,
            Phase::ResearchDiscovery,
            Phase::TechniqueSelection,
            Phase::VariantGeneration,
            Phase::Benchmarking,
        ]
    }

    /// Get the display label for the phase tab.
    pub fn label(&self) -> &'static str {
        match self {
            Phase::ProjectIntake => "1.Intake",
            Phase::ResearchDiscovery => "2.Research",
            Phase::TechniqueSelection => "3.Techniques",
            Phase::VariantGeneration => "4.Build",
            Phase::Benchmarking => "5.Benchmark",
        }
    }

    /// Get the next phase, if any.
    pub fn next(&self) -> Option<Phase> {
        match self {
            Phase::ProjectIntake => Some(Phase::ResearchDiscovery),
            Phase::ResearchDiscovery => Some(Phase::TechniqueSelection),
            Phase::TechniqueSelection => Some(Phase::VariantGeneration),
            Phase::VariantGeneration => Some(Phase::Benchmarking),
            Phase::Benchmarking => None,
        }
    }

    /// Get the previous phase, if any.
    pub fn prev(&self) -> Option<Phase> {
        match self {
            Phase::ProjectIntake => None,
            Phase::ResearchDiscovery => Some(Phase::ProjectIntake),
            Phase::TechniqueSelection => Some(Phase::ResearchDiscovery),
            Phase::VariantGeneration => Some(Phase::TechniqueSelection),
            Phase::Benchmarking => Some(Phase::VariantGeneration),
        }
    }

    /// Numeric index (0-based).
    pub fn index(&self) -> usize {
        match self {
            Phase::ProjectIntake => 0,
            Phase::ResearchDiscovery => 1,
            Phase::TechniqueSelection => 2,
            Phase::VariantGeneration => 3,
            Phase::Benchmarking => 4,
        }
    }
}

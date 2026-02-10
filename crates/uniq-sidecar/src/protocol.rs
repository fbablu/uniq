//! Request and response types for communicating with the Python sidecar.
//! These types mirror the Python Pydantic models exactly.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ── Health ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

// ── Project Analysis ────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AnalyzeProjectRequest {
    pub path: PathBuf,
    pub description: String,
}

// Response is uniq_core::project::ProjectProfile (deserialized directly)

// ── Paper Search ────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SearchPapersRequest {
    pub queries: Vec<String>,
    pub max_results: usize,
    pub year_min: Option<u16>,
    pub year_max: Option<u16>,
    pub prefer_open_access: bool,
}

// Response is Vec<uniq_core::research::PaperMeta>

// ── Technique Extraction ────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ExtractTechniqueRequest {
    pub pdf_url: String,
    pub paper_id: String,
    pub paper_title: String,
    pub project_summary: String,
    pub user_request: String,
}

// Response is uniq_core::research::TechniqueCard

// ── Variant Generation ──────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct GenerateVariantRequest {
    pub technique: uniq_core::research::TechniqueCard,
    pub project: uniq_core::project::ProjectProfile,
    pub branch_name: String,
}

#[derive(Debug, Deserialize)]
pub struct GenerateVariantResponse {
    pub success: bool,
    pub modified_files: Vec<String>,
    pub new_dependencies: Vec<String>,
    pub error: Option<String>,
}

// ── Variant Merge ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct MergeVariantsRequest {
    pub variant_a_branch: String,
    pub variant_a_technique: serde_json::Value,
    pub variant_b_branch: String,
    pub variant_b_technique: serde_json::Value,
    pub blend_a: u8,
    pub blend_b: u8,
    pub project: uniq_core::project::ProjectProfile,
    pub target_branch: String,
}

// Response is GenerateVariantResponse

// ── Benchmark ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct RunBenchmarkRequest {
    pub variant_branches: Vec<String>,
    pub project_path: PathBuf,
    pub metrics: Vec<String>,
    pub timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
pub struct RunBenchmarkResponse {
    pub results: HashMap<String, uniq_core::benchmark::ExecutionMetrics>,
}

// ── LLM Judge ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct LlmJudgeRequest {
    pub variant_branches: Vec<String>,
    pub project_path: PathBuf,
    pub user_request: String,
}

#[derive(Debug, Deserialize)]
pub struct LlmJudgeResponse {
    pub scores: HashMap<String, uniq_core::benchmark::JudgeScores>,
}

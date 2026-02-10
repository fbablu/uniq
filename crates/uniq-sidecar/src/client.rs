//! HTTP client for communicating with the Python sidecar.

use reqwest::Client;
use std::collections::HashMap;
use tracing::{debug, instrument};

use uniq_core::benchmark::{ExecutionMetrics, JudgeScores};
use uniq_core::project::ProjectProfile;
use uniq_core::research::{PaperMeta, TechniqueCard};

use crate::protocol::*;

/// Client for the Python sidecar API.
pub struct SidecarClient {
    client: Client,
    base_url: String,
}

impl SidecarClient {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client, base_url }
    }

    /// Check sidecar health.
    #[instrument(skip(self))]
    pub async fn health(&self) -> anyhow::Result<HealthResponse> {
        let url = format!("{}/api/health", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let health: HealthResponse = resp.json().await?;
        Ok(health)
    }

    /// Analyze a project directory.
    #[instrument(skip(self))]
    pub async fn analyze_project(
        &self,
        path: std::path::PathBuf,
        description: String,
    ) -> anyhow::Result<ProjectProfile> {
        let url = format!("{}/api/analyze-project", self.base_url);
        let req = AnalyzeProjectRequest { path, description };
        let resp = self.client.post(&url).json(&req).send().await?;
        let profile: ProjectProfile = resp.error_for_status()?.json().await?;
        debug!(
            "Project analyzed: {} files, {} languages",
            profile.file_count,
            profile.languages.len()
        );
        Ok(profile)
    }

    /// Search for academic papers.
    #[instrument(skip(self))]
    pub async fn search_papers(
        &self,
        queries: Vec<String>,
        max_results: usize,
        year_min: Option<u16>,
        year_max: Option<u16>,
        prefer_open_access: bool,
    ) -> anyhow::Result<Vec<PaperMeta>> {
        let url = format!("{}/api/search-papers", self.base_url);
        let req = SearchPapersRequest {
            queries,
            max_results,
            year_min,
            year_max,
            prefer_open_access,
        };
        let resp = self.client.post(&url).json(&req).send().await?;
        let papers: Vec<PaperMeta> = resp.error_for_status()?.json().await?;
        debug!("Found {} papers", papers.len());
        Ok(papers)
    }

    /// Extract a technique card from a paper PDF.
    #[instrument(skip(self, project_summary))]
    pub async fn extract_technique(
        &self,
        pdf_url: String,
        paper_id: String,
        paper_title: String,
        project_summary: String,
        user_request: String,
    ) -> anyhow::Result<TechniqueCard> {
        let url = format!("{}/api/extract-technique", self.base_url);
        let req = ExtractTechniqueRequest {
            pdf_url,
            paper_id,
            paper_title,
            project_summary,
            user_request,
        };
        let resp = self.client.post(&url).json(&req).send().await?;
        let technique: TechniqueCard = resp.error_for_status()?.json().await?;
        debug!("Extracted technique: {}", technique.name);
        Ok(technique)
    }

    /// Generate a variant by applying a technique to the project.
    #[instrument(skip(self, technique, project))]
    pub async fn generate_variant(
        &self,
        technique: TechniqueCard,
        project: ProjectProfile,
        branch_name: String,
    ) -> anyhow::Result<GenerateVariantResponse> {
        let url = format!("{}/api/generate-variant", self.base_url);
        let req = GenerateVariantRequest {
            technique,
            project,
            branch_name,
        };
        let resp = self.client.post(&url).json(&req).send().await?;
        let result: GenerateVariantResponse = resp.error_for_status()?.json().await?;
        Ok(result)
    }

    /// Merge two variants with specified blend ratios.
    #[instrument(skip(self, project))]
    pub async fn merge_variants(
        &self,
        variant_a_branch: String,
        variant_a_technique: serde_json::Value,
        variant_b_branch: String,
        variant_b_technique: serde_json::Value,
        blend_a: u8,
        blend_b: u8,
        project: ProjectProfile,
        target_branch: String,
    ) -> anyhow::Result<GenerateVariantResponse> {
        let url = format!("{}/api/merge-variants", self.base_url);
        let req = MergeVariantsRequest {
            variant_a_branch,
            variant_a_technique,
            variant_b_branch,
            variant_b_technique,
            blend_a,
            blend_b,
            project,
            target_branch,
        };
        let resp = self.client.post(&url).json(&req).send().await?;
        let result: GenerateVariantResponse = resp.error_for_status()?.json().await?;
        Ok(result)
    }

    /// Run benchmarks on variant branches.
    #[instrument(skip(self))]
    pub async fn run_benchmark(
        &self,
        variant_branches: Vec<String>,
        project_path: std::path::PathBuf,
        metrics: Vec<String>,
        timeout_seconds: u64,
    ) -> anyhow::Result<HashMap<String, ExecutionMetrics>> {
        let url = format!("{}/api/run-benchmark", self.base_url);
        let req = RunBenchmarkRequest {
            variant_branches,
            project_path,
            metrics,
            timeout_seconds,
        };
        let resp = self.client.post(&url).json(&req).send().await?;
        let result: RunBenchmarkResponse = resp.error_for_status()?.json().await?;
        Ok(result.results)
    }

    /// Run LLM-as-judge evaluation on variants.
    #[instrument(skip(self))]
    pub async fn llm_judge(
        &self,
        variant_branches: Vec<String>,
        project_path: std::path::PathBuf,
        user_request: String,
    ) -> anyhow::Result<HashMap<String, JudgeScores>> {
        let url = format!("{}/api/llm-judge", self.base_url);
        let req = LlmJudgeRequest {
            variant_branches,
            project_path,
            user_request,
        };
        let resp = self.client.post(&url).json(&req).send().await?;
        let result: LlmJudgeResponse = resp.error_for_status()?.json().await?;
        Ok(result.scores)
    }

    /// Request graceful shutdown.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let url = format!("{}/api/shutdown", self.base_url);
        let _ = self.client.post(&url).send().await;
        Ok(())
    }
}

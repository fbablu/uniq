use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Individual metric result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub higher_is_better: bool,
}

/// Results from automated code execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// Whether the project built/compiled successfully.
    pub build_success: bool,

    /// Build error output if failed.
    pub build_error: Option<String>,

    /// Test pass rate (0.0 - 1.0).
    pub test_pass_rate: Option<f64>,

    /// Number of tests passed / total.
    pub tests_passed: Option<u32>,
    pub tests_total: Option<u32>,

    /// Runtime in milliseconds.
    pub runtime_ms: Option<f64>,

    /// Peak memory usage in MB.
    pub memory_mb: Option<f64>,

    /// Custom metrics (model accuracy, F1, RMSE, etc.).
    pub custom_metrics: HashMap<String, MetricValue>,
}

/// Scores from LLM-as-judge evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeScores {
    /// Code quality (0-10).
    pub code_quality: f64,

    /// Novelty of the approach (0-10).
    pub novelty: f64,

    /// Feasibility for production use (0-10).
    pub feasibility: f64,

    /// Alignment with the user's stated goal (0-10).
    pub goal_alignment: f64,

    /// Completeness of the implementation (0-10).
    pub completeness: f64,

    /// Overall weighted score (0-10).
    pub overall: f64,

    /// Free-form explanation from the judge.
    pub explanation: String,
}

/// User-provided rating for a variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRating {
    /// Overall score (1-5 stars).
    pub stars: u8,

    /// Free-form notes.
    pub notes: String,
}

/// Complete benchmark results for a single variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    /// Automated execution metrics.
    pub execution: Option<ExecutionMetrics>,

    /// LLM-as-judge scores.
    pub judge: Option<JudgeScores>,

    /// User rating.
    pub user_rating: Option<UserRating>,

    /// Composite score (weighted combination of all available scores, 0-100).
    pub composite_score: Option<f64>,
}

impl BenchmarkResults {
    /// Calculate the composite score from available sub-scores.
    pub fn compute_composite(&mut self) {
        let mut total = 0.0;
        let mut weight = 0.0;

        // Execution: 40% weight
        if let Some(ref exec) = self.execution {
            let mut exec_score = 0.0;
            if exec.build_success {
                exec_score += 30.0;
            }
            if let Some(rate) = exec.test_pass_rate {
                exec_score += rate * 70.0;
            }
            total += exec_score * 0.4;
            weight += 0.4;
        }

        // Judge: 40% weight
        if let Some(ref judge) = self.judge {
            total += (judge.overall / 10.0) * 100.0 * 0.4;
            weight += 0.4;
        }

        // User: 20% weight
        if let Some(ref user) = self.user_rating {
            total += (user.stars as f64 / 5.0) * 100.0 * 0.2;
            weight += 0.2;
        }

        if weight > 0.0 {
            self.composite_score = Some(total / weight);
        }
    }
}

impl Default for BenchmarkResults {
    fn default() -> Self {
        Self {
            execution: None,
            judge: None,
            user_rating: None,
            composite_score: None,
        }
    }
}

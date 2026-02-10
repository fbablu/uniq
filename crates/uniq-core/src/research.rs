use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Metadata about an academic paper from Semantic Scholar or arXiv.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperMeta {
    /// Unique identifier (e.g., "arxiv:2106.15928" or Semantic Scholar ID).
    pub id: String,

    /// Paper title.
    pub title: String,

    /// List of author names.
    pub authors: Vec<String>,

    /// Publication year.
    pub year: Option<u16>,

    /// Publication date if available.
    pub published_date: Option<NaiveDate>,

    /// Abstract text.
    pub abstract_text: String,

    /// Number of citations.
    pub citation_count: Option<u32>,

    /// URL to the paper's page.
    pub url: String,

    /// Direct URL to an open-access PDF, if available.
    pub pdf_url: Option<String>,

    /// DOI identifier, if available.
    pub doi: Option<String>,

    /// Source of this paper record.
    pub source: PaperSource,

    /// Fields of study / categories.
    pub fields: Vec<String>,

    /// Relevance score computed by our system (0.0 - 1.0).
    pub relevance_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaperSource {
    SemanticScholar,
    ArXiv,
}

/// A structured technique extracted from a research paper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechniqueCard {
    /// Human-readable name for this technique.
    pub name: String,

    /// Reference to the source paper.
    pub paper_id: String,

    /// Paper title for display.
    pub paper_title: String,

    /// Detailed description of the methodology.
    pub methodology: String,

    /// Key algorithmic or architectural components.
    pub key_components: Vec<String>,

    /// What data format / input this technique expects.
    pub required_data_format: String,

    /// Estimated implementation complexity.
    pub implementation_complexity: Complexity,

    /// Hardware requirements or recommendations.
    pub hardware_requirements: String,

    /// External dependencies needed (libraries, packages).
    pub dependencies: Vec<String>,

    /// How well this technique fits the user's project (0.0 - 1.0).
    pub relevance_score: f64,

    /// Suggested approach for integrating into the user's project.
    pub integration_approach: String,

    /// Whether the user has selected this technique for variant generation.
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Complexity {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for Complexity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Complexity::Low => write!(f, "Low"),
            Complexity::Medium => write!(f, "Medium"),
            Complexity::High => write!(f, "High"),
        }
    }
}

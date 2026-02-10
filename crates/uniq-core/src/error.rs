use thiserror::Error;

#[derive(Error, Debug)]
pub enum UniqError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Project analysis error: {0}")]
    ProjectAnalysis(String),

    #[error("Research discovery error: {0}")]
    Research(String),

    #[error("PDF extraction error: {0}")]
    PdfExtraction(String),

    #[error("Variant generation error: {0}")]
    VariantGeneration(String),

    #[error("Variant merge error: {0}")]
    VariantMerge(String),

    #[error("Benchmark error: {0}")]
    Benchmark(String),

    #[error("Git operation error: {0}")]
    Git(#[from] git2::Error),

    #[error("Sidecar communication error: {0}")]
    Sidecar(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, UniqError>;

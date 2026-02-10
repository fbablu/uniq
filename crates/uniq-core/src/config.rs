use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniqConfig {
    #[serde(default)]
    pub api_keys: ApiKeysConfig,

    #[serde(default)]
    pub search: SearchConfig,

    #[serde(default)]
    pub generation: GenerationConfig,

    #[serde(default)]
    pub benchmark: BenchmarkConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeysConfig {
    #[serde(default)]
    pub anthropic: String,

    #[serde(default)]
    pub semantic_scholar: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_max_papers")]
    pub max_papers: usize,

    #[serde(default = "default_top_techniques")]
    pub top_techniques: usize,

    #[serde(default = "default_year_range")]
    pub year_range: [u16; 2],

    #[serde(default = "default_true")]
    pub prefer_open_access: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    #[serde(default = "default_claude_model")]
    pub claude_model: String,

    #[serde(default = "default_max_tokens")]
    pub max_tokens_per_variant: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    #[serde(default = "default_metrics")]
    pub metrics: Vec<String>,
}

fn default_max_papers() -> usize {
    500
}
fn default_top_techniques() -> usize {
    10
}
fn default_year_range() -> [u16; 2] {
    [2015, 2026]
}
fn default_true() -> bool {
    true
}
fn default_claude_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}
fn default_max_tokens() -> usize {
    8192
}
fn default_timeout() -> u64 {
    300
}
fn default_metrics() -> Vec<String> {
    vec![
        "build_success".to_string(),
        "test_pass".to_string(),
        "runtime_ms".to_string(),
        "memory_mb".to_string(),
    ]
}

impl Default for UniqConfig {
    fn default() -> Self {
        Self {
            api_keys: ApiKeysConfig::default(),
            search: SearchConfig::default(),
            generation: GenerationConfig::default(),
            benchmark: BenchmarkConfig::default(),
        }
    }
}

impl Default for ApiKeysConfig {
    fn default() -> Self {
        Self {
            anthropic: String::new(),
            semantic_scholar: String::new(),
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_papers: default_max_papers(),
            top_techniques: default_top_techniques(),
            year_range: default_year_range(),
            prefer_open_access: default_true(),
        }
    }
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            claude_model: default_claude_model(),
            max_tokens_per_variant: default_max_tokens(),
        }
    }
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: default_timeout(),
            metrics: default_metrics(),
        }
    }
}

impl UniqConfig {
    /// Load config from ~/.config/uniq/config.toml, creating defaults if missing.
    pub fn load() -> crate::error::Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path).map_err(|e| {
                crate::error::UniqError::Config(format!("Failed to read config: {e}"))
            })?;
            let config: UniqConfig = toml::from_str(&contents).map_err(|e| {
                crate::error::UniqError::Config(format!("Failed to parse config: {e}"))
            })?;
            Ok(config)
        } else {
            let config = UniqConfig::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save config to disk.
    pub fn save(&self) -> crate::error::Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self).map_err(|e| {
            crate::error::UniqError::Config(format!("Failed to serialize config: {e}"))
        })?;
        std::fs::write(&config_path, contents)?;
        Ok(())
    }

    /// Get the config file path.
    pub fn config_path() -> crate::error::Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or_else(|| {
            crate::error::UniqError::Config("Could not determine config directory".into())
        })?;
        Ok(config_dir.join("uniq").join("config.toml"))
    }
}

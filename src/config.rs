use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub transcription: TranscriptionConfig,
    #[serde(default)]
    pub summarization: SummarizationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub data_dir: Option<String>,
    #[serde(default = "default_max_downloads")]
    pub max_concurrent_downloads: usize,
    #[serde(default = "default_true")]
    pub auto_cleanup_audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    #[serde(default)]
    pub backend: TranscriptionBackend,
    #[serde(default = "default_whisper_model")]
    pub whisper_model: String,
    pub language: Option<String>,
    /// Initial prompt to guide transcription style and vocabulary
    pub initial_prompt: Option<String>,
    /// Percentage of CPU threads to use (1-100, default 80)
    #[serde(default = "default_cpu_percent")]
    pub cpu_percent: u32,
    /// Post-process transcription with OpenCC Chinese conversion (e.g. "s2twp" for Simplified → Taiwan Traditional)
    pub chinese_conversion: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionBackend {
    #[default]
    Local,
    Api,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizationConfig {
    #[serde(default = "default_api_base_url")]
    pub api_base_url: String,
    #[serde(default = "default_api_key_env")]
    pub api_key_env: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    pub system_prompt: Option<String>,
}

fn default_max_downloads() -> usize {
    3
}
fn default_true() -> bool {
    true
}
fn default_whisper_model() -> String {
    "base".to_string()
}
fn default_cpu_percent() -> u32 {
    80
}
fn default_api_base_url() -> String {
    "https://generativelanguage.googleapis.com/v1beta/openai".to_string()
}
fn default_api_key_env() -> String {
    "GEMINI_API_KEY".to_string()
}
fn default_model() -> String {
    "gemini-2.0-flash".to_string()
}
fn default_max_tokens() -> u32 {
    4096
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            data_dir: None,
            max_concurrent_downloads: default_max_downloads(),
            auto_cleanup_audio: true,
        }
    }
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            backend: TranscriptionBackend::default(),
            whisper_model: default_whisper_model(),
            language: None,
            initial_prompt: None,
            cpu_percent: default_cpu_percent(),
            chinese_conversion: None,
        }
    }
}

impl Default for SummarizationConfig {
    fn default() -> Self {
        Self {
            api_base_url: default_api_base_url(),
            api_key_env: default_api_key_env(),
            model: default_model(),
            max_tokens: default_max_tokens(),
            system_prompt: None,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config at {}", path.display()))?;
            let config: Self = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config at {}", path.display()))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("podcast-summarize").join("config.toml"))
    }

    pub fn data_dir(&self) -> Result<PathBuf> {
        if let Some(ref dir) = self.general.data_dir {
            let path = PathBuf::from(shellexpand(dir));
            Ok(path)
        } else {
            let data_dir = dirs::data_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
            Ok(data_dir.join("podcast-summarize"))
        }
    }

    pub fn db_path(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join("db.sqlite3"))
    }

    pub fn audio_dir(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join("audio"))
    }

    pub fn transcript_dir(&self) -> Result<PathBuf> {
        Ok(self.data_dir()?.join("transcripts"))
    }

    pub fn api_key(&self) -> Result<String> {
        std::env::var(&self.summarization.api_key_env).with_context(|| {
            format!(
                "API key not set. Set the {} environment variable or update config with:\n  podcast-summarize config set api_key_env <ENV_VAR_NAME>",
                self.summarization.api_key_env
            )
        })
    }
}

fn shellexpand(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest).to_string_lossy().to_string();
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_transcription_config() {
        let config = TranscriptionConfig::default();
        assert_eq!(config.whisper_model, "base");
        assert!(config.language.is_none());
        assert!(config.initial_prompt.is_none());
        assert_eq!(config.cpu_percent, 80);
    }

    #[test]
    fn default_summarization_config() {
        let config = SummarizationConfig::default();
        assert_eq!(config.api_key_env, "GEMINI_API_KEY");
        assert_eq!(config.model, "gemini-2.0-flash");
        assert_eq!(config.max_tokens, 4096);
        assert!(config.system_prompt.is_none());
    }

    #[test]
    fn default_general_config() {
        let config = GeneralConfig::default();
        assert!(config.data_dir.is_none());
        assert_eq!(config.max_concurrent_downloads, 3);
        assert!(config.auto_cleanup_audio);
    }

    #[test]
    fn toml_roundtrip() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            parsed.transcription.whisper_model,
            config.transcription.whisper_model
        );
        assert_eq!(parsed.summarization.model, config.summarization.model);
        assert_eq!(
            parsed.general.auto_cleanup_audio,
            config.general.auto_cleanup_audio
        );
    }

    #[test]
    fn partial_toml_uses_defaults() {
        let toml_str = r#"
[transcription]
whisper_model = "large"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.transcription.whisper_model, "large");
        // Rest should be defaults
        assert_eq!(config.transcription.cpu_percent, 80);
        assert_eq!(config.summarization.model, "gemini-2.0-flash");
        assert_eq!(config.general.max_concurrent_downloads, 3);
    }

    #[test]
    fn empty_toml_parses_to_defaults() {
        let config: AppConfig = toml::from_str("").unwrap();
        assert_eq!(config.transcription.whisper_model, "base");
        assert_eq!(config.summarization.max_tokens, 4096);
        assert!(config.general.auto_cleanup_audio);
    }

    #[test]
    fn shellexpand_without_tilde() {
        let result = shellexpand("/absolute/path");
        assert_eq!(result, "/absolute/path");
    }
}

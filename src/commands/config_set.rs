use anyhow::Result;

use crate::config::AppConfig;

pub fn run(key: &str, value: &str) -> Result<()> {
    let mut config = AppConfig::load()?;

    match key {
        "cpu_percent" => {
            let v: u32 = value.parse().map_err(|_| anyhow::anyhow!("Invalid number"))?;
            if !(1..=100).contains(&v) {
                anyhow::bail!("cpu_percent must be between 1 and 100");
            }
            config.transcription.cpu_percent = v;
        }
        "whisper_model" => {
            config.transcription.whisper_model = value.to_string();
        }
        "language" => {
            config.transcription.language = Some(value.to_string());
        }
        "initial_prompt" => {
            config.transcription.initial_prompt = Some(value.to_string());
        }
        "api_base_url" => {
            config.summarization.api_base_url = value.to_string();
        }
        "api_key_env" => {
            config.summarization.api_key_env = value.to_string();
        }
        "model" => {
            config.summarization.model = value.to_string();
        }
        "max_tokens" => {
            let v: u32 = value.parse().map_err(|_| anyhow::anyhow!("Invalid number"))?;
            config.summarization.max_tokens = v;
        }
        "auto_cleanup_audio" => {
            let v: bool = value.parse().map_err(|_| anyhow::anyhow!("Expected true or false"))?;
            config.general.auto_cleanup_audio = v;
        }
        _ => {
            anyhow::bail!(
                "Unknown config key: {key}\n\nAvailable keys:\n  cpu_percent, whisper_model, language, initial_prompt,\n  api_base_url, api_key_env, model, max_tokens, auto_cleanup_audio"
            );
        }
    }

    config.save()?;
    println!("Set {key} = {value}");
    Ok(())
}

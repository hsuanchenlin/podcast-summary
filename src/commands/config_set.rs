use anyhow::Result;

use crate::config::AppConfig;

pub fn run(key: &str, value: &str) -> Result<()> {
    let mut config = AppConfig::load()?;
    validate_and_apply(&mut config, key, value)?;
    config.save()?;
    println!("Set {key} = {value}");
    Ok(())
}

fn validate_and_apply(config: &mut AppConfig, key: &str, value: &str) -> Result<()> {
    match key {
        "cpu_percent" => {
            let v: u32 = value
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid number"))?;
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
            let v: u32 = value
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid number"))?;
            config.summarization.max_tokens = v;
        }
        "auto_cleanup_audio" => {
            let v: bool = value
                .parse()
                .map_err(|_| anyhow::anyhow!("Expected true or false"))?;
            config.general.auto_cleanup_audio = v;
        }
        "chinese_conversion" => {
            let valid = [
                "s2t", "s2tw", "s2twp", "s2hk", "t2s", "tw2s", "tw2sp", "hk2s", "t2tw", "t2hk",
            ];
            let lower = value.to_lowercase();
            if !valid.contains(&lower.as_str()) {
                anyhow::bail!(
                    "Invalid chinese_conversion variant: {value}\nValid values: {}",
                    valid.join(", ")
                );
            }
            config.transcription.chinese_conversion = Some(lower);
        }
        _ => {
            anyhow::bail!(
                "Unknown config key: {key}\n\nAvailable keys:\n  cpu_percent, whisper_model, language, initial_prompt, chinese_conversion,\n  api_base_url, api_key_env, model, max_tokens, auto_cleanup_audio"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> AppConfig {
        AppConfig::default()
    }

    #[test]
    fn cpu_percent_valid_min() {
        let mut c = default_config();
        validate_and_apply(&mut c, "cpu_percent", "1").unwrap();
        assert_eq!(c.transcription.cpu_percent, 1);
    }

    #[test]
    fn cpu_percent_valid_max() {
        let mut c = default_config();
        validate_and_apply(&mut c, "cpu_percent", "100").unwrap();
        assert_eq!(c.transcription.cpu_percent, 100);
    }

    #[test]
    fn cpu_percent_zero_fails() {
        let mut c = default_config();
        assert!(validate_and_apply(&mut c, "cpu_percent", "0").is_err());
    }

    #[test]
    fn cpu_percent_101_fails() {
        let mut c = default_config();
        assert!(validate_and_apply(&mut c, "cpu_percent", "101").is_err());
    }

    #[test]
    fn cpu_percent_non_numeric_fails() {
        let mut c = default_config();
        assert!(validate_and_apply(&mut c, "cpu_percent", "abc").is_err());
    }

    #[test]
    fn bool_parsing_true() {
        let mut c = default_config();
        validate_and_apply(&mut c, "auto_cleanup_audio", "false").unwrap();
        assert!(!c.general.auto_cleanup_audio);
        validate_and_apply(&mut c, "auto_cleanup_audio", "true").unwrap();
        assert!(c.general.auto_cleanup_audio);
    }

    #[test]
    fn bool_parsing_invalid() {
        let mut c = default_config();
        assert!(validate_and_apply(&mut c, "auto_cleanup_audio", "yes").is_err());
    }

    #[test]
    fn string_fields() {
        let mut c = default_config();
        validate_and_apply(&mut c, "whisper_model", "large-v3").unwrap();
        assert_eq!(c.transcription.whisper_model, "large-v3");

        validate_and_apply(&mut c, "language", "zh").unwrap();
        assert_eq!(c.transcription.language.as_deref(), Some("zh"));

        validate_and_apply(&mut c, "model", "gpt-4o").unwrap();
        assert_eq!(c.summarization.model, "gpt-4o");
    }

    #[test]
    fn max_tokens_valid() {
        let mut c = default_config();
        validate_and_apply(&mut c, "max_tokens", "8192").unwrap();
        assert_eq!(c.summarization.max_tokens, 8192);
    }

    #[test]
    fn max_tokens_invalid() {
        let mut c = default_config();
        assert!(validate_and_apply(&mut c, "max_tokens", "not_a_number").is_err());
    }

    #[test]
    fn unknown_key_fails() {
        let mut c = default_config();
        let err = validate_and_apply(&mut c, "nonexistent", "value").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Unknown config key"));
        assert!(msg.contains("nonexistent"));
    }
}

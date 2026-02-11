use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::audio;
use crate::config::AppConfig;

const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Transcribe an audio file to text using local whisper.cpp.
pub fn transcribe(audio_path: &Path, config: &AppConfig, progress: Arc<AtomicI32>) -> Result<String> {
    let model_path = ensure_model(config)?;
    let samples = audio::decode_to_whisper_format(audio_path)
        .with_context(|| format!("Failed to decode audio: {}", audio_path.display()))?;

    tracing::info!(
        "Transcribing {} samples ({:.1}s of audio)",
        samples.len(),
        samples.len() as f64 / 16000.0
    );

    let ctx = WhisperContext::new_with_params(
        model_path.to_str().unwrap_or_default(),
        WhisperContextParameters::default(),
    )
    .map_err(|e| anyhow::anyhow!("Failed to load whisper model: {e}"))?;

    let mut state = ctx
        .create_state()
        .map_err(|e| anyhow::anyhow!("Failed to create whisper state: {e}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    if let Some(ref lang) = config.transcription.language {
        params.set_language(Some(lang));
    } else {
        params.set_language(Some("en"));
    }

    if let Some(ref prompt) = config.transcription.initial_prompt {
        params.set_initial_prompt(prompt);
    }

    let pct = config.transcription.cpu_percent.clamp(1, 100);
    let n_threads = std::thread::available_parallelism()
        .map(|n| ((n.get() as u32 * pct / 100).max(1)) as i32)
        .unwrap_or(4);
    params.set_n_threads(n_threads);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_progress_callback_safe(move |pct| {
        progress.store(pct, Ordering::Relaxed);
    });

    state
        .full(params, &samples)
        .map_err(|e| anyhow::anyhow!("Whisper transcription failed: {e}"))?;

    let n_segments = state.full_n_segments();

    let mut transcript = String::new();
    for i in 0..n_segments {
        if let Some(segment) = state.get_segment(i) {
            if let Ok(text) = segment.to_str_lossy() {
                transcript.push_str(&text);
            }
        }
    }

    Ok(transcript.trim().to_string())
}

/// Ensure the whisper model file exists, downloading if needed.
fn ensure_model(config: &AppConfig) -> Result<PathBuf> {
    let model_name = &config.transcription.whisper_model;
    let filename = format!("ggml-{model_name}.bin");

    let model_dir = config.data_dir()?.join("models");
    std::fs::create_dir_all(&model_dir)?;
    let model_path = model_dir.join(&filename);

    if model_path.exists() {
        return Ok(model_path);
    }

    eprintln!(
        "Whisper model '{model_name}' not found. Downloading to {}...",
        model_path.display()
    );

    download_model(&filename, &model_path)?;

    Ok(model_path)
}

fn download_model(filename: &str, dest: &Path) -> Result<()> {
    let url = format!("{MODEL_BASE_URL}/{filename}");

    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?
        .get(&url)
        .header("User-Agent", "podcast-summarize/0.1.0")
        .send()
        .with_context(|| format!("Failed to download model from {url}"))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Failed to download model: HTTP {} from {url}",
            response.status()
        );
    }

    let total = response.content_length().unwrap_or(0);
    let pb = indicatif::ProgressBar::new(total);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.cyan/dim}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("##-"),
    );

    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp)?;

    let mut downloaded = 0u64;
    let mut reader = response;
    let mut buf = [0u8; 8192];
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf)?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n])?;
        downloaded += n as u64;
        pb.set_position(downloaded);
    }

    pb.finish_and_clear();
    std::fs::rename(&tmp, dest)?;

    eprintln!("  Model downloaded successfully.");
    Ok(())
}

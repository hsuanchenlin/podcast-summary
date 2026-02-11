use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};

pub async fn download_episode(
    client: &reqwest::Client,
    audio_url: &str,
    output_dir: &Path,
    podcast_id: i64,
) -> Result<PathBuf> {
    let podcast_dir = output_dir.join(podcast_id.to_string());
    std::fs::create_dir_all(&podcast_dir)?;

    // Derive filename from URL
    let filename = audio_url
        .rsplit('/')
        .next()
        .unwrap_or("episode.mp3")
        .split('?')
        .next()
        .unwrap_or("episode.mp3");
    let dest = podcast_dir.join(filename);

    if dest.exists() {
        return Ok(dest);
    }

    let response = client
        .get(audio_url)
        .header("User-Agent", "podcast-summarize/0.1.0")
        .send()
        .await
        .with_context(|| format!("Failed to download: {audio_url}"))?;

    let total_size = response.content_length().unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("    {msg} [{bar:30.cyan/dim}] {bytes}/{total_bytes} {bytes_per_sec}")
            .unwrap()
            .progress_chars("##-"),
    );
    pb.set_message(truncate_filename(filename, 30));

    let tmp_dest = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp_dest).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| "Error reading download stream")?;
        pb.inc(chunk.len() as u64);
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }

    pb.finish_and_clear();

    // Rename .part to final filename
    tokio::fs::rename(&tmp_dest, &dest).await?;
    Ok(dest)
}

fn truncate_filename(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        name.to_string()
    } else {
        let truncated: String = name.chars().take(max - 3).collect();
        format!("{truncated}...")
    }
}

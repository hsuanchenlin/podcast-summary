use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::Semaphore;

use crate::config::AppConfig;
use crate::db::Database;
use crate::models::EpisodeStatus;
use crate::{download, feed, summarize, transcribe};

pub async fn run(
    name: Option<&str>,
    episode_id: Option<i64>,
    download_only: bool,
    redo: bool,
    config: &AppConfig,
) -> Result<()> {
    let db = Database::open(&config.db_path()?)?;
    let client = reqwest::Client::new();

    // If a specific episode ID is given, process it first
    if let Some(ep_id) = episode_id {
        if redo {
            clear_episode_results(&db, ep_id)?;
        }
        run_single_episode(&db, &client, ep_id, download_only, config).await?;
        if name.is_none() {
            return Ok(());
        }
        println!();
    }

    let podcasts = if let Some(name) = name {
        let p = if let Ok(id) = name.parse::<i64>() {
            db.get_podcast(id).ok()
        } else {
            db.find_podcast_by_name(name)?
        }
        .ok_or_else(|| anyhow::anyhow!("No podcast matching \"{name}\" found"))?;
        vec![p]
    } else {
        db.list_podcasts()?
    };

    if podcasts.is_empty() {
        println!("No subscriptions. Add one with: podcast-summarize add <RSS_URL>");
        return Ok(());
    }

    // Phase 1: Fetch feeds and discover new episodes
    println!("Checking feeds...");
    let mut all_new_episodes = Vec::new();

    for podcast in &podcasts {
        match feed::sync_feed(&client, &db, podcast).await {
            Ok(new_eps) => {
                if new_eps.is_empty() {
                    println!("  {}: up to date", podcast.title);
                } else {
                    println!("  {}: {} new episode(s)", podcast.title, new_eps.len());
                    all_new_episodes.extend(new_eps);
                }
            }
            Err(e) => {
                eprintln!("  {}: failed to fetch feed: {e}", podcast.title);
            }
        }
    }

    if all_new_episodes.is_empty() {
        println!("\nAll feeds up to date.");
        return Ok(());
    }

    // Phase 2: Download new episodes
    let downloaded = download_episodes(&db, &client, &all_new_episodes, config).await?;

    if download_only || downloaded.is_empty() {
        println!("\nDone. {} episode(s) downloaded.", downloaded.len());
        return Ok(());
    }

    // Phase 3: Transcribe
    let transcribed = transcribe_episodes(&db, &downloaded, config).await?;

    if transcribed.is_empty() {
        println!("\nNo episodes transcribed successfully.");
        return Ok(());
    }

    // Phase 4: Summarize
    summarize_episodes(&db, &client, &transcribed, config).await?;

    // Cleanup audio if configured
    if config.general.auto_cleanup_audio {
        for (_, audio_path) in &downloaded {
            if audio_path.exists() {
                let _ = std::fs::remove_file(audio_path);
            }
        }
    }

    println!("\nSync complete.");
    Ok(())
}

/// Clear old transcript and summary so they get regenerated.
fn clear_episode_results(db: &Database, ep_id: i64) -> Result<()> {
    let episode = db.get_episode(ep_id)?;

    // Delete transcript file
    if let Some(ref path) = episode.transcript_path {
        let p = std::path::Path::new(path);
        if p.exists() {
            std::fs::remove_file(p)?;
        }
    }
    db.clear_episode_transcript(ep_id)?;

    // Delete summary from DB
    db.delete_summary_by_episode(ep_id)?;

    println!("  Cleared old transcript and summary for episode #{ep_id}.");
    Ok(())
}

/// Process a single episode by ID through the full pipeline.
async fn run_single_episode(
    db: &Database,
    client: &reqwest::Client,
    ep_id: i64,
    download_only: bool,
    config: &AppConfig,
) -> Result<()> {
    let episode = db.get_episode(ep_id)?;
    let podcast = db.get_podcast(episode.podcast_id)?;

    println!("Processing: \"{}\" ({})", episode.title, podcast.title);

    // Download if needed
    let audio_path = if let Some(ref existing) = episode.audio_path {
        let p = std::path::Path::new(existing);
        if p.exists() {
            println!("  Audio already downloaded.");
            PathBuf::from(existing)
        } else {
            println!("  Downloading...");
            let audio_dir = config.audio_dir()?;
            let path = download::download_episode(
                client,
                &episode.audio_url,
                &audio_dir,
                episode.podcast_id,
            )
            .await?;
            let path_str = path.to_string_lossy().to_string();
            db.update_episode_audio_path(ep_id, &path_str)?;
            println!("  Downloaded.");
            path
        }
    } else {
        println!("  Downloading...");
        let audio_dir = config.audio_dir()?;
        let path =
            download::download_episode(client, &episode.audio_url, &audio_dir, episode.podcast_id)
                .await?;
        let path_str = path.to_string_lossy().to_string();
        db.update_episode_audio_path(ep_id, &path_str)?;
        println!("  Downloaded.");
        path
    };

    if download_only {
        println!("\nDone (download only).");
        return Ok(());
    }

    // Transcribe if needed
    let transcript = if let Some(ref existing) = episode.transcript_path {
        let p = std::path::Path::new(existing);
        if p.exists() {
            println!("  Transcript already exists.");
            std::fs::read_to_string(existing)?
        } else {
            // Transcript path set but file missing, re-transcribe
            let pb = ProgressBar::new(100);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("  [{bar:30.cyan/dim}] {pos}% Transcribing...")
                    .unwrap()
                    .progress_chars("##-"),
            );

            let progress = Arc::new(std::sync::atomic::AtomicI32::new(0));
            let progress_clone = progress.clone();

            let audio_path_clone = audio_path.clone();
            let config_clone = config.clone();
            let handle = tokio::task::spawn_blocking(move || {
                transcribe::transcribe(&audio_path_clone, &config_clone, progress_clone)
            });

            loop {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                let pct = progress.load(std::sync::atomic::Ordering::Relaxed);
                pb.set_position(pct.max(0) as u64);
                if handle.is_finished() {
                    break;
                }
            }
            let result = handle.await??;

            pb.set_position(100);
            pb.finish_and_clear();

            let transcript_dir = config.transcript_dir()?;
            let transcript_file = transcript_dir
                .join(episode.podcast_id.to_string())
                .join(format!("{ep_id}.txt"));
            if let Some(parent) = transcript_file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&transcript_file, &result)?;
            db.update_episode_transcript_path(ep_id, &transcript_file.to_string_lossy())?;

            let word_count = count_text_length(&result);
            println!("  Transcribed ({word_count} words).");
            result
        }
    } else {
        let pb = ProgressBar::new(100);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [{bar:30.cyan/dim}] {pos}% Transcribing...")
                .unwrap()
                .progress_chars("##-"),
        );

        let progress = Arc::new(std::sync::atomic::AtomicI32::new(0));
        let progress_clone = progress.clone();

        let audio_path_clone = audio_path.clone();
        let config_clone = config.clone();
        let handle = tokio::task::spawn_blocking(move || {
            transcribe::transcribe(&audio_path_clone, &config_clone, progress_clone)
        });

        // Poll progress until transcription finishes
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let pct = progress.load(std::sync::atomic::Ordering::Relaxed);
            pb.set_position(pct.max(0) as u64);
            if handle.is_finished() {
                break;
            }
        }
        let result = handle.await??;

        pb.set_position(100);
        pb.finish_and_clear();

        // Save transcript
        let transcript_dir = config.transcript_dir()?;
        let transcript_file = transcript_dir
            .join(episode.podcast_id.to_string())
            .join(format!("{ep_id}.txt"));
        if let Some(parent) = transcript_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&transcript_file, &result)?;
        db.update_episode_transcript_path(ep_id, &transcript_file.to_string_lossy())?;

        let word_count = count_text_length(&result);
        println!("  Transcribed ({word_count} words).");
        result
    };

    // Summarize
    if db.get_summary_by_episode(ep_id)?.is_some() {
        println!("  Summary already exists. Use `pod-sum show {ep_id}` to read it.");
        return Ok(());
    }

    let api_key = config.api_key()?;

    let spinner_style = ProgressStyle::default_spinner()
        .template("  {spinner} Summarizing...")
        .unwrap();
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style);
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let result = summarize::generate_summary(
        client,
        &config.summarization.api_base_url,
        &api_key,
        &config.summarization.model,
        config.summarization.max_tokens,
        config.summarization.system_prompt.as_deref(),
        &transcript,
    )
    .await?;

    db.insert_summary(
        ep_id,
        &result.content,
        &result.model,
        result.prompt_tokens,
        result.output_tokens,
    )?;
    pb.finish_and_clear();
    println!("  Summarized.");

    // Cleanup audio if configured
    if config.general.auto_cleanup_audio && audio_path.exists() {
        let _ = std::fs::remove_file(&audio_path);
    }

    println!("\nDone! Run `podcast-summarize show {ep_id}` to read the summary.");
    Ok(())
}

// --- Helper functions for batch processing ---

async fn download_episodes(
    db: &Database,
    client: &reqwest::Client,
    episodes: &[crate::models::Episode],
    config: &AppConfig,
) -> Result<Vec<(i64, PathBuf)>> {
    let audio_dir = config.audio_dir()?;
    let semaphore = Arc::new(Semaphore::new(config.general.max_concurrent_downloads));
    let _mp = MultiProgress::new();

    println!("\nDownloading {} episode(s)...", episodes.len());

    let mut download_tasks = Vec::new();
    for episode in episodes {
        let permit = semaphore.clone().acquire_owned().await?;
        let client = client.clone();
        let audio_url = episode.audio_url.clone();
        let audio_dir = audio_dir.clone();
        let podcast_id = episode.podcast_id;
        let ep_id = episode.id;
        let title = episode.title.clone();

        download_tasks.push(tokio::spawn(async move {
            let result =
                download::download_episode(&client, &audio_url, &audio_dir, podcast_id).await;
            drop(permit);
            (ep_id, title, result)
        }));
    }

    let mut downloaded = Vec::new();
    for task in download_tasks {
        let (ep_id, title, result) = task.await?;
        match result {
            Ok(path) => {
                let path_str = path.to_string_lossy().to_string();
                db.update_episode_audio_path(ep_id, &path_str)?;
                println!("  Downloaded: {title}");
                downloaded.push((ep_id, path));
            }
            Err(e) => {
                eprintln!("  Failed to download \"{title}\": {e}");
                db.update_episode_status(ep_id, &EpisodeStatus::Failed(format!("download: {e}")))?;
            }
        }
    }
    Ok(downloaded)
}

async fn transcribe_episodes(
    db: &Database,
    downloaded: &[(i64, PathBuf)],
    config: &AppConfig,
) -> Result<Vec<(i64, String)>> {
    let transcript_dir = config.transcript_dir()?;
    std::fs::create_dir_all(&transcript_dir)?;

    println!("\nTranscribing {} episode(s)...", downloaded.len());

    let bar_style = ProgressStyle::default_bar()
        .template("  [{bar:30.cyan/dim}] {pos}% {msg}")
        .unwrap()
        .progress_chars("##-");

    let mut transcribed = Vec::new();

    for (ep_id, audio_path) in downloaded {
        let episode = db.get_episode(*ep_id)?;

        let pb = ProgressBar::new(100);
        pb.set_style(bar_style.clone());
        pb.set_message(format!("Transcribing: {}", episode.title));

        let progress = Arc::new(std::sync::atomic::AtomicI32::new(0));
        let progress_clone = progress.clone();

        let audio_path_clone = audio_path.clone();
        let config_clone = config.clone();
        let handle = tokio::task::spawn_blocking(move || {
            transcribe::transcribe(&audio_path_clone, &config_clone, progress_clone)
        });

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let pct = progress.load(std::sync::atomic::Ordering::Relaxed);
            pb.set_position(pct.max(0) as u64);
            if handle.is_finished() {
                break;
            }
        }
        let result = handle.await?;

        match result {
            Ok(transcript) => {
                let transcript_file = transcript_dir
                    .join(episode.podcast_id.to_string())
                    .join(format!("{}.txt", ep_id));
                if let Some(parent) = transcript_file.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&transcript_file, &transcript)?;

                let path_str = transcript_file.to_string_lossy().to_string();
                db.update_episode_transcript_path(*ep_id, &path_str)?;

                let word_count = count_text_length(&transcript);
                pb.finish_with_message(format!(
                    "Transcribed: {} ({} words)",
                    episode.title, word_count,
                ));

                transcribed.push((*ep_id, transcript));
            }
            Err(e) => {
                pb.finish_with_message(format!("Failed: {}", episode.title));
                eprintln!("    Error transcribing: {e}");
                db.update_episode_status(
                    *ep_id,
                    &EpisodeStatus::Failed(format!("transcribe: {e}")),
                )?;
            }
        }
    }
    Ok(transcribed)
}

async fn summarize_episodes(
    db: &Database,
    client: &reqwest::Client,
    transcribed: &[(i64, String)],
    config: &AppConfig,
) -> Result<()> {
    let api_key = match config.api_key() {
        Ok(key) => key,
        Err(e) => {
            eprintln!("\nSkipping summarization: {e}");
            println!("Transcripts are saved. Re-run sync after setting API key.");
            return Ok(());
        }
    };

    println!("\nSummarizing {} episode(s)...", transcribed.len());

    let spinner_style = ProgressStyle::default_spinner()
        .template("  {spinner} {msg}")
        .unwrap();

    for (ep_id, transcript) in transcribed {
        let episode = db.get_episode(*ep_id)?;

        let pb = ProgressBar::new_spinner();
        pb.set_style(spinner_style.clone());
        pb.set_message(format!("Summarizing: {}", episode.title));
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        match summarize::generate_summary(
            client,
            &config.summarization.api_base_url,
            &api_key,
            &config.summarization.model,
            config.summarization.max_tokens,
            config.summarization.system_prompt.as_deref(),
            transcript,
        )
        .await
        {
            Ok(result) => {
                db.insert_summary(
                    *ep_id,
                    &result.content,
                    &result.model,
                    result.prompt_tokens,
                    result.output_tokens,
                )?;
                pb.finish_with_message(format!("Summarized: {} [done]", episode.title));
            }
            Err(e) => {
                pb.finish_with_message(format!("Summary failed: {}", episode.title));
                eprintln!("    Error: {e}");
                db.update_episode_status(
                    *ep_id,
                    &EpisodeStatus::Failed(format!("summarize: {e}")),
                )?;
            }
        }
    }
    Ok(())
}

/// Count text length: characters for CJK-heavy text, words for others.
pub fn count_text_length(s: &str) -> usize {
    let cjk_count = s.chars().filter(|c| is_cjk(*c)).count();
    let total_chars = s.chars().filter(|c| !c.is_whitespace()).count();
    if total_chars > 0 && cjk_count * 100 / total_chars > 30 {
        total_chars
    } else {
        s.split_whitespace().count()
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' |
        '\u{3400}'..='\u{4DBF}' |
        '\u{F900}'..='\u{FAFF}' |
        '\u{3000}'..='\u{303F}' |
        '\u{FF00}'..='\u{FFEF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_text_length_english() {
        assert_eq!(count_text_length("hello world foo bar"), 4);
    }

    #[test]
    fn count_text_length_cjk() {
        // All CJK chars - should count characters (excluding whitespace)
        let text = "今天天氣很好我們去散步";
        let result = count_text_length(text);
        assert_eq!(result, 11);
    }

    #[test]
    fn count_text_length_mixed_below_threshold() {
        // Mostly English with a few CJK chars (below 30% threshold)
        let text = "This is a long English sentence with one 字";
        let result = count_text_length(text);
        // CJK ratio is low, so word count
        assert_eq!(result, text.split_whitespace().count());
    }

    #[test]
    fn count_text_length_empty() {
        assert_eq!(count_text_length(""), 0);
    }

    #[test]
    fn count_text_length_whitespace_only() {
        assert_eq!(count_text_length("   \n\t  "), 0);
    }

    #[test]
    fn is_cjk_chinese_char() {
        assert!(is_cjk('中'));
        assert!(is_cjk('國'));
    }
}

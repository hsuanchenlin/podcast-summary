use anyhow::Result;

use crate::config::AppConfig;
use crate::db::Database;

pub fn run(episode_id: i64, transcript: bool, config: &AppConfig) -> Result<()> {
    let db = Database::open(&config.db_path()?)?;

    let episode = db.get_episode(episode_id)?;
    let podcast = db.get_podcast(episode.podcast_id)?;

    println!();
    println!("  {}", "═".repeat(60));
    println!("  {} - {}", episode.title, podcast.title);
    if let Some(date) = episode.published_at {
        let duration = episode
            .duration_secs
            .map(|d| {
                let h = d / 3600;
                let m = (d % 3600) / 60;
                if h > 0 {
                    format!("{h}h{m:02}m")
                } else {
                    format!("{m}m")
                }
            })
            .unwrap_or_default();
        println!(
            "  Published: {} | Duration: {}",
            date.format("%Y-%m-%d"),
            duration
        );
    }
    println!("  {}", "═".repeat(60));

    if transcript {
        match &episode.transcript_path {
            Some(path) if std::path::Path::new(path).exists() => {
                let content = std::fs::read_to_string(path)?;
                let word_count = super::sync::count_text_length(&content);
                println!();
                println!("{}", indent(&content, 2));
                println!();
                println!("  {}", "─".repeat(60));
                println!("  Transcript: {word_count} chars");
            }
            _ => {
                println!();
                println!("  No transcript yet. Run: podcast-summarize sync -e {episode_id}");
            }
        }
    } else {
        match db.get_summary_by_episode(episode_id)? {
            Some(summary) => {
                println!();
                println!("{}", indent(&summary.content, 2));
                println!();
                println!("  {}", "─".repeat(60));
                println!(
                    "  Model: {} | Generated: {}",
                    summary.model,
                    summary.created_at.format("%Y-%m-%d %H:%M")
                );
                if let (Some(pt), Some(ot)) = (summary.prompt_tokens, summary.output_tokens) {
                    println!("  Tokens: {pt} in / {ot} out");
                }
            }
            None => {
                println!();
                println!("  No summary yet. Run: podcast-summarize sync -e {episode_id}");
                println!("  Status: {:?}", episode.status);
            }
        }
    }
    println!("  {}", "═".repeat(60));
    println!();

    Ok(())
}

fn indent(s: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    s.lines()
        .map(|l| format!("{prefix}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

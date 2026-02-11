use anyhow::Result;

use crate::config::AppConfig;
use crate::db::Database;
use crate::feed;

pub async fn run(url: &str, config: &AppConfig) -> Result<()> {
    let db = Database::open(&config.db_path()?)?;
    let client = reqwest::Client::new();

    // Check for duplicate
    if let Some(existing) = db.find_podcast_by_url(url)? {
        println!("Already subscribed to \"{}\"", existing.title);
        return Ok(());
    }

    println!("Fetching feed...");
    let feed_info = feed::fetch_feed(&client, url).await?;

    let podcast = db.insert_podcast(
        url,
        &feed_info.title,
        feed_info.website_url.as_deref(),
        feed_info.description.as_deref(),
    )?;

    // Insert all discovered episodes
    let mut count = 0;
    for entry in &feed_info.entries {
        db.insert_episode(
            podcast.id,
            &entry.guid,
            &entry.title,
            entry.description.as_deref(),
            &entry.audio_url,
            entry.published_at,
            entry.duration_secs,
        )?;
        count += 1;
    }

    println!();
    println!("  Added: {}", podcast.title);
    if let Some(ref url) = podcast.website_url {
        println!("  Website: {url}");
    }
    println!("  Episodes: {count}");
    if let Some(latest) = feed_info.entries.first() {
        let date = latest
            .published_at
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        println!("  Latest: \"{}\" ({date})", latest.title);
    }
    println!();

    Ok(())
}

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

pub struct FeedEntry {
    pub guid: String,
    pub title: String,
    pub description: Option<String>,
    pub audio_url: String,
    pub published_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
}

pub struct FeedInfo {
    pub title: String,
    pub website_url: Option<String>,
    pub description: Option<String>,
    pub entries: Vec<FeedEntry>,
}

pub async fn fetch_feed(client: &reqwest::Client, url: &str) -> Result<FeedInfo> {
    let response = client
        .get(url)
        .header("User-Agent", "podcast-summarize/0.1.0")
        .send()
        .await
        .with_context(|| format!("Failed to fetch feed: {url}"))?;

    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("Failed to read feed body: {url}"))?;

    let feed =
        feed_rs::parser::parse(&bytes[..]).map_err(|e| crate::error::AppError::FeedParse {
            url: url.to_string(),
            msg: e.to_string(),
        })?;

    let title = feed
        .title
        .map(|t| t.content)
        .unwrap_or_else(|| "Untitled".to_string());
    let website_url = feed.links.first().map(|l| l.href.clone());
    let description = feed.description.map(|d| d.content);

    let entries = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            // Find audio URL from media content
            let audio_url_from_media = entry.media.iter().flat_map(|m| &m.content).find_map(|c| {
                let is_audio = c
                    .content_type
                    .as_ref()
                    .is_some_and(|mime| mime.ty() == "audio");
                let url_looks_audio = c.url.as_ref().is_some_and(|u| {
                    let u = u.as_str();
                    u.ends_with(".mp3") || u.ends_with(".m4a") || u.ends_with(".ogg")
                });
                if is_audio || url_looks_audio {
                    c.url.as_ref().map(|u| u.to_string())
                } else {
                    None
                }
            });

            let audio_url = audio_url_from_media.or_else(|| {
                // Fallback: check entry links for audio enclosures
                entry
                    .links
                    .iter()
                    .find(|l| {
                        l.media_type
                            .as_ref()
                            .is_some_and(|m| m.starts_with("audio/"))
                            || l.rel.as_deref() == Some("enclosure")
                    })
                    .map(|l| l.href.clone())
            })?;

            let guid = entry.id;
            let title = entry
                .title
                .map(|t| t.content)
                .unwrap_or_else(|| "Untitled".to_string());
            let description = entry.summary.map(|s| s.content);
            let published_at = entry.published.or(entry.updated);

            // Parse duration from media content
            let duration_secs = entry
                .media
                .iter()
                .flat_map(|m| &m.content)
                .find_map(|c| c.duration.map(|d| d.as_secs() as i64));

            Some(FeedEntry {
                guid,
                title,
                description,
                audio_url,
                published_at,
                duration_secs,
            })
        })
        .collect();

    Ok(FeedInfo {
        title,
        website_url,
        description,
        entries,
    })
}

/// Sync a feed: fetch new episodes and insert them into the database.
/// Returns the number of new episodes found.
pub async fn sync_feed(
    client: &reqwest::Client,
    db: &crate::db::Database,
    podcast: &crate::models::Podcast,
) -> Result<Vec<crate::models::Episode>> {
    let feed = fetch_feed(client, &podcast.feed_url).await?;

    let mut new_episodes = Vec::new();
    for entry in feed.entries {
        let id = db.insert_episode(
            podcast.id,
            &entry.guid,
            &entry.title,
            entry.description.as_deref(),
            &entry.audio_url,
            entry.published_at,
            entry.duration_secs,
        )?;
        // insert_episode uses INSERT OR IGNORE, so id=0 means it already existed
        if id > 0 {
            let episode = db.get_episode(id)?;
            new_episodes.push(episode);
        }
    }

    db.update_last_checked(podcast.id)?;
    Ok(new_episodes)
}

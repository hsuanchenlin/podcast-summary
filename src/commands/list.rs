use anyhow::Result;

use crate::config::AppConfig;
use crate::db::Database;

pub fn run(name: Option<&str>, config: &AppConfig) -> Result<()> {
    let db = Database::open(&config.db_path()?)?;

    if let Some(name) = name {
        // Show episodes for a specific podcast (by ID or name)
        let podcast = if let Ok(id) = name.parse::<i64>() {
            db.get_podcast(id).ok()
        } else {
            db.find_podcast_by_name(name)?
        }
        .ok_or_else(|| anyhow::anyhow!("No podcast matching \"{name}\" found"))?;

        let episodes = db.list_episodes(podcast.id)?;
        println!();
        println!("  {} ({} episodes)", podcast.title, episodes.len());
        println!("  {}", "─".repeat(50));

        for ep in &episodes {
            let date = ep
                .published_at
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "          ".to_string());

            let duration = ep.duration_secs.map(format_duration).unwrap_or_default();

            let status = match &ep.status {
                crate::models::EpisodeStatus::New => "[new]",
                crate::models::EpisodeStatus::Downloaded => "[dl]",
                crate::models::EpisodeStatus::Transcribed => "[txt]",
                crate::models::EpisodeStatus::Summarized => "[done]",
                crate::models::EpisodeStatus::Failed(_) => "[err]",
            };

            println!(
                "  #{:<5} {:<40} {} {:>6} {}",
                ep.id,
                truncate(&ep.title, 40),
                date,
                duration,
                status,
            );
        }
        println!();
    } else {
        // Show all podcasts
        let podcasts = db.list_podcasts()?;
        if podcasts.is_empty() {
            println!("No subscriptions yet. Add one with: podcast-summarize add <RSS_URL>");
            return Ok(());
        }

        println!();
        println!(
            "  {:<4} {:<30} {:>8} {:>8} {:>12}",
            "ID", "PODCAST", "EPISODES", "NEW", "LAST CHECKED"
        );
        println!("  {}", "─".repeat(66));

        for p in &podcasts {
            let total = db.episode_count(p.id)?;
            let new = db.episode_count_by_status(p.id, "new")?;
            let last_checked = p
                .last_checked
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "never".to_string());

            println!(
                "  {:<4} {:<30} {:>8} {:>8} {:>12}",
                p.id,
                truncate(&p.title, 30),
                total,
                new,
                last_checked,
            );
        }
        println!();
    }

    Ok(())
}

fn format_duration(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{h}h{m:02}m")
    } else {
        format!("{m}m")
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{truncated}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_duration_minutes_only() {
        assert_eq!(format_duration(300), "5m");
        assert_eq!(format_duration(59 * 60), "59m");
    }

    #[test]
    fn format_duration_hours_and_minutes() {
        assert_eq!(format_duration(3600), "1h00m");
        assert_eq!(format_duration(3661), "1h01m");
        assert_eq!(format_duration(7200 + 1800), "2h30m");
    }

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(0), "0m");
    }

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_string() {
        assert_eq!(truncate("hello world!", 8), "hello...");
    }

    #[test]
    fn truncate_unicode() {
        // Unicode chars should be counted properly
        let s = "你好世界測試文字串";
        let result = truncate(s, 6);
        assert_eq!(result, "你好世...");
    }
}

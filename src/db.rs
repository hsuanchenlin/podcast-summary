use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::models::{Episode, EpisodeStatus, Podcast, Summary};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open database at {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS podcasts (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                title       TEXT NOT NULL,
                feed_url    TEXT NOT NULL UNIQUE,
                website_url TEXT,
                description TEXT,
                last_checked TEXT,
                added_at    TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS episodes (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                podcast_id      INTEGER NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
                guid            TEXT NOT NULL,
                title           TEXT NOT NULL,
                description     TEXT,
                audio_url       TEXT NOT NULL,
                published_at    TEXT,
                duration_secs   INTEGER,
                status          TEXT NOT NULL DEFAULT 'new',
                fail_reason     TEXT,
                audio_path      TEXT,
                transcript_path TEXT,
                discovered_at   TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(podcast_id, guid)
            );

            CREATE TABLE IF NOT EXISTS summaries (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                episode_id    INTEGER NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
                content       TEXT NOT NULL,
                model         TEXT NOT NULL,
                prompt_tokens INTEGER,
                output_tokens INTEGER,
                created_at    TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_episodes_podcast_id ON episodes(podcast_id);
            CREATE INDEX IF NOT EXISTS idx_episodes_status ON episodes(status);
            CREATE INDEX IF NOT EXISTS idx_summaries_episode_id ON summaries(episode_id);",
        )?;
        Ok(())
    }

    // --- Podcasts ---

    pub fn insert_podcast(&self, feed_url: &str, title: &str, website_url: Option<&str>, description: Option<&str>) -> Result<Podcast> {
        self.conn.execute(
            "INSERT INTO podcasts (feed_url, title, website_url, description) VALUES (?1, ?2, ?3, ?4)",
            params![feed_url, title, website_url, description],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_podcast(id)
    }

    pub fn get_podcast(&self, id: i64) -> Result<Podcast> {
        self.conn.query_row(
            "SELECT id, title, feed_url, website_url, description, last_checked, added_at FROM podcasts WHERE id = ?1",
            params![id],
            |row| Ok(Podcast {
                id: row.get(0)?,
                title: row.get(1)?,
                feed_url: row.get(2)?,
                website_url: row.get(3)?,
                description: row.get(4)?,
                last_checked: row.get::<_, Option<String>>(5)?.and_then(|s| s.parse().ok()),
                added_at: row.get::<_, String>(6)?.parse().unwrap_or_else(|_| Utc::now()),
            }),
        ).with_context(|| format!("Podcast with id {} not found", id))
    }

    pub fn find_podcast_by_name(&self, name: &str) -> Result<Option<Podcast>> {
        let pattern = format!("%{}%", name);
        let mut stmt = self.conn.prepare(
            "SELECT id, title, feed_url, website_url, description, last_checked, added_at FROM podcasts WHERE title LIKE ?1 COLLATE NOCASE",
        )?;
        let mut rows = stmt.query(params![pattern])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Podcast {
                id: row.get(0)?,
                title: row.get(1)?,
                feed_url: row.get(2)?,
                website_url: row.get(3)?,
                description: row.get(4)?,
                last_checked: row.get::<_, Option<String>>(5)?.and_then(|s| s.parse().ok()),
                added_at: row.get::<_, String>(6)?.parse().unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn find_podcast_by_url(&self, url: &str) -> Result<Option<Podcast>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, feed_url, website_url, description, last_checked, added_at FROM podcasts WHERE feed_url = ?1",
        )?;
        let mut rows = stmt.query(params![url])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Podcast {
                id: row.get(0)?,
                title: row.get(1)?,
                feed_url: row.get(2)?,
                website_url: row.get(3)?,
                description: row.get(4)?,
                last_checked: row.get::<_, Option<String>>(5)?.and_then(|s| s.parse().ok()),
                added_at: row.get::<_, String>(6)?.parse().unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_podcasts(&self) -> Result<Vec<Podcast>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, feed_url, website_url, description, last_checked, added_at FROM podcasts ORDER BY title",
        )?;
        let podcasts = stmt.query_map([], |row| {
            Ok(Podcast {
                id: row.get(0)?,
                title: row.get(1)?,
                feed_url: row.get(2)?,
                website_url: row.get(3)?,
                description: row.get(4)?,
                last_checked: row.get::<_, Option<String>>(5)?.and_then(|s| s.parse().ok()),
                added_at: row.get::<_, String>(6)?.parse().unwrap_or_else(|_| Utc::now()),
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(podcasts)
    }

    pub fn delete_podcast(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM podcasts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update_last_checked(&self, podcast_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE podcasts SET last_checked = datetime('now') WHERE id = ?1",
            params![podcast_id],
        )?;
        Ok(())
    }

    // --- Episodes ---

    pub fn insert_episode(
        &self,
        podcast_id: i64,
        guid: &str,
        title: &str,
        description: Option<&str>,
        audio_url: &str,
        published_at: Option<DateTime<Utc>>,
        duration_secs: Option<i64>,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO episodes (podcast_id, guid, title, description, audio_url, published_at, duration_secs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                podcast_id,
                guid,
                title,
                description,
                audio_url,
                published_at.map(|d| d.to_rfc3339()),
                duration_secs,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_episode(&self, id: i64) -> Result<Episode> {
        self.conn.query_row(
            "SELECT id, podcast_id, guid, title, description, audio_url, published_at, duration_secs, status, fail_reason, audio_path, transcript_path, discovered_at
             FROM episodes WHERE id = ?1",
            params![id],
            Self::map_episode,
        ).with_context(|| format!("Episode with id {} not found", id))
    }

    pub fn list_episodes(&self, podcast_id: i64) -> Result<Vec<Episode>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, podcast_id, guid, title, description, audio_url, published_at, duration_secs, status, fail_reason, audio_path, transcript_path, discovered_at
             FROM episodes WHERE podcast_id = ?1 ORDER BY published_at DESC",
        )?;
        let episodes = stmt.query_map(params![podcast_id], Self::map_episode)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(episodes)
    }

    pub fn list_episodes_by_status(&self, status: &str) -> Result<Vec<Episode>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, podcast_id, guid, title, description, audio_url, published_at, duration_secs, status, fail_reason, audio_path, transcript_path, discovered_at
             FROM episodes WHERE status = ?1 ORDER BY published_at DESC",
        )?;
        let episodes = stmt.query_map(params![status], Self::map_episode)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(episodes)
    }

    pub fn update_episode_status(&self, id: i64, status: &EpisodeStatus) -> Result<()> {
        self.conn.execute(
            "UPDATE episodes SET status = ?1, fail_reason = ?2 WHERE id = ?3",
            params![status.as_str(), status.fail_reason(), id],
        )?;
        Ok(())
    }

    pub fn update_episode_audio_path(&self, id: i64, path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE episodes SET audio_path = ?1, status = 'downloaded' WHERE id = ?2",
            params![path, id],
        )?;
        Ok(())
    }

    pub fn update_episode_transcript_path(&self, id: i64, path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE episodes SET transcript_path = ?1, status = 'transcribed' WHERE id = ?2",
            params![path, id],
        )?;
        Ok(())
    }

    pub fn episode_count(&self, podcast_id: i64) -> Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM episodes WHERE podcast_id = ?1",
            params![podcast_id],
            |row| row.get(0),
        ).map_err(Into::into)
    }

    pub fn episode_count_by_status(&self, podcast_id: i64, status: &str) -> Result<i64> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM episodes WHERE podcast_id = ?1 AND status = ?2",
            params![podcast_id, status],
            |row| row.get(0),
        ).map_err(Into::into)
    }

    // --- Summaries ---

    pub fn insert_summary(
        &self,
        episode_id: i64,
        content: &str,
        model: &str,
        prompt_tokens: Option<i64>,
        output_tokens: Option<i64>,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO summaries (episode_id, content, model, prompt_tokens, output_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![episode_id, content, model, prompt_tokens, output_tokens],
        )?;
        self.conn.execute(
            "UPDATE episodes SET status = 'summarized' WHERE id = ?1",
            params![episode_id],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_summary_by_episode(&self, episode_id: i64) -> Result<Option<Summary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, episode_id, content, model, prompt_tokens, output_tokens, created_at
             FROM summaries WHERE episode_id = ?1 ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![episode_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Summary {
                id: row.get(0)?,
                episode_id: row.get(1)?,
                content: row.get(2)?,
                model: row.get(3)?,
                prompt_tokens: row.get(4)?,
                output_tokens: row.get(5)?,
                created_at: row.get::<_, String>(6)?.parse().unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn delete_summary_by_episode(&self, episode_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM summaries WHERE episode_id = ?1",
            params![episode_id],
        )?;
        Ok(())
    }

    pub fn clear_episode_transcript(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE episodes SET transcript_path = NULL, status = 'downloaded' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    fn map_episode(row: &rusqlite::Row<'_>) -> rusqlite::Result<Episode> {
        let status_str: String = row.get(8)?;
        let fail_reason: Option<String> = row.get(9)?;
        Ok(Episode {
            id: row.get(0)?,
            podcast_id: row.get(1)?,
            guid: row.get(2)?,
            title: row.get(3)?,
            description: row.get(4)?,
            audio_url: row.get(5)?,
            published_at: row.get::<_, Option<String>>(6)?.and_then(|s| s.parse().ok()),
            duration_secs: row.get(7)?,
            status: EpisodeStatus::from_db(&status_str, fail_reason.as_deref()),
            audio_path: row.get(10)?,
            transcript_path: row.get(11)?,
            discovered_at: row.get::<_, String>(12)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }
}

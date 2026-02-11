use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};

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

    pub fn insert_podcast(
        &self,
        feed_url: &str,
        title: &str,
        website_url: Option<&str>,
        description: Option<&str>,
    ) -> Result<Podcast> {
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
                last_checked: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|s| s.parse().ok()),
                added_at: row
                    .get::<_, String>(6)?
                    .parse()
                    .unwrap_or_else(|_| Utc::now()),
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
                last_checked: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|s| s.parse().ok()),
                added_at: row
                    .get::<_, String>(6)?
                    .parse()
                    .unwrap_or_else(|_| Utc::now()),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn list_podcasts(&self) -> Result<Vec<Podcast>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, feed_url, website_url, description, last_checked, added_at FROM podcasts ORDER BY title",
        )?;
        let podcasts = stmt
            .query_map([], |row| {
                Ok(Podcast {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    feed_url: row.get(2)?,
                    website_url: row.get(3)?,
                    description: row.get(4)?,
                    last_checked: row
                        .get::<_, Option<String>>(5)?
                        .and_then(|s| s.parse().ok()),
                    added_at: row
                        .get::<_, String>(6)?
                        .parse()
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(podcasts)
    }

    pub fn delete_podcast(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM podcasts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update_last_checked(&self, podcast_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE podcasts SET last_checked = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
            params![podcast_id],
        )?;
        Ok(())
    }

    // --- Episodes ---

    #[allow(clippy::too_many_arguments)]
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
        let episodes = stmt
            .query_map(params![podcast_id], Self::map_episode)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(episodes)
    }

    #[allow(dead_code)]
    pub fn list_episodes_by_status(&self, status: &str) -> Result<Vec<Episode>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, podcast_id, guid, title, description, audio_url, published_at, duration_secs, status, fail_reason, audio_path, transcript_path, discovered_at
             FROM episodes WHERE status = ?1 ORDER BY published_at DESC",
        )?;
        let episodes = stmt
            .query_map(params![status], Self::map_episode)?
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
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM episodes WHERE podcast_id = ?1",
                params![podcast_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn episode_count_by_status(&self, podcast_id: i64, status: &str) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM episodes WHERE podcast_id = ?1 AND status = ?2",
                params![podcast_id, status],
                |row| row.get(0),
            )
            .map_err(Into::into)
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
                created_at: row
                    .get::<_, String>(6)?
                    .parse()
                    .unwrap_or_else(|_| Utc::now()),
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
            published_at: row
                .get::<_, Option<String>>(6)?
                .and_then(|s| s.parse().ok()),
            duration_secs: row.get(7)?,
            status: EpisodeStatus::from_db(&status_str, fail_reason.as_deref()),
            audio_path: row.get(10)?,
            transcript_path: row.get(11)?,
            discovered_at: row
                .get::<_, String>(12)?
                .parse()
                .unwrap_or_else(|_| Utc::now()),
        })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    // --- Podcast CRUD ---

    #[test]
    fn insert_and_get_podcast() {
        let db = test_db();
        let p = db
            .insert_podcast("https://example.com/feed.xml", "Test Pod", None, None)
            .unwrap();
        assert_eq!(p.title, "Test Pod");
        assert_eq!(p.feed_url, "https://example.com/feed.xml");

        let fetched = db.get_podcast(p.id).unwrap();
        assert_eq!(fetched.title, "Test Pod");
    }

    #[test]
    fn insert_podcast_with_optional_fields() {
        let db = test_db();
        let p = db
            .insert_podcast(
                "https://ex.com/feed",
                "Pod",
                Some("https://ex.com"),
                Some("A podcast"),
            )
            .unwrap();
        assert_eq!(p.website_url.as_deref(), Some("https://ex.com"));
        assert_eq!(p.description.as_deref(), Some("A podcast"));
    }

    #[test]
    fn duplicate_feed_url_fails() {
        let db = test_db();
        db.insert_podcast("https://example.com/feed.xml", "First", None, None)
            .unwrap();
        let result = db.insert_podcast("https://example.com/feed.xml", "Second", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn find_podcast_by_name_partial_match() {
        let db = test_db();
        db.insert_podcast("https://ex.com/f1", "Rust Weekly", None, None)
            .unwrap();
        db.insert_podcast("https://ex.com/f2", "Go Monthly", None, None)
            .unwrap();

        let found = db.find_podcast_by_name("rust").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Rust Weekly");
    }

    #[test]
    fn find_podcast_by_name_no_match() {
        let db = test_db();
        db.insert_podcast("https://ex.com/f1", "Rust Weekly", None, None)
            .unwrap();
        let found = db.find_podcast_by_name("python").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn find_podcast_by_url() {
        let db = test_db();
        db.insert_podcast("https://ex.com/feed.xml", "Test", None, None)
            .unwrap();
        let found = db.find_podcast_by_url("https://ex.com/feed.xml").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Test");
    }

    #[test]
    fn find_podcast_by_url_not_found() {
        let db = test_db();
        let found = db.find_podcast_by_url("https://nope.com/feed.xml").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn list_podcasts_ordered_by_title() {
        let db = test_db();
        db.insert_podcast("https://ex.com/b", "Bravo", None, None)
            .unwrap();
        db.insert_podcast("https://ex.com/a", "Alpha", None, None)
            .unwrap();
        db.insert_podcast("https://ex.com/c", "Charlie", None, None)
            .unwrap();

        let list = db.list_podcasts().unwrap();
        let titles: Vec<&str> = list.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(titles, vec!["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn list_podcasts_empty() {
        let db = test_db();
        let list = db.list_podcasts().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn delete_podcast() {
        let db = test_db();
        let p = db
            .insert_podcast("https://ex.com/feed", "ToDelete", None, None)
            .unwrap();
        db.delete_podcast(p.id).unwrap();
        assert!(db.get_podcast(p.id).is_err());
    }

    #[test]
    fn update_last_checked() {
        let db = test_db();
        let p = db
            .insert_podcast("https://ex.com/feed", "Pod", None, None)
            .unwrap();
        assert!(p.last_checked.is_none());
        db.update_last_checked(p.id).unwrap();
        let updated = db.get_podcast(p.id).unwrap();
        assert!(updated.last_checked.is_some());
    }

    // --- Episode CRUD ---

    fn insert_test_podcast(db: &Database) -> Podcast {
        db.insert_podcast("https://ex.com/feed", "Test Pod", None, None)
            .unwrap()
    }

    #[test]
    fn insert_and_get_episode() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep_id = db
            .insert_episode(
                p.id,
                "guid-1",
                "Episode 1",
                None,
                "https://ex.com/ep1.mp3",
                None,
                Some(3600),
            )
            .unwrap();
        assert!(ep_id > 0);

        let ep = db.get_episode(ep_id).unwrap();
        assert_eq!(ep.title, "Episode 1");
        assert_eq!(ep.guid, "guid-1");
        assert_eq!(ep.duration_secs, Some(3600));
        assert_eq!(ep.status, EpisodeStatus::New);
    }

    #[test]
    fn duplicate_guid_ignored() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        db.insert_episode(
            p.id,
            "guid-1",
            "First",
            None,
            "https://ex.com/1.mp3",
            None,
            None,
        )
        .unwrap();
        // INSERT OR IGNORE - duplicate guid same podcast is ignored
        db.insert_episode(
            p.id,
            "guid-1",
            "Duplicate",
            None,
            "https://ex.com/2.mp3",
            None,
            None,
        )
        .unwrap();
        let episodes = db.list_episodes(p.id).unwrap();
        assert_eq!(episodes.len(), 1);
        assert_eq!(episodes[0].title, "First");
    }

    #[test]
    fn list_episodes_for_podcast() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        db.insert_episode(p.id, "g1", "Ep 1", None, "https://ex.com/1.mp3", None, None)
            .unwrap();
        db.insert_episode(p.id, "g2", "Ep 2", None, "https://ex.com/2.mp3", None, None)
            .unwrap();

        let episodes = db.list_episodes(p.id).unwrap();
        assert_eq!(episodes.len(), 2);
    }

    #[test]
    fn list_episodes_by_status() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep1 = db
            .insert_episode(p.id, "g1", "Ep 1", None, "https://ex.com/1.mp3", None, None)
            .unwrap();
        db.insert_episode(p.id, "g2", "Ep 2", None, "https://ex.com/2.mp3", None, None)
            .unwrap();
        db.update_episode_audio_path(ep1, "/tmp/audio.mp3").unwrap();

        let downloaded = db.list_episodes_by_status("downloaded").unwrap();
        assert_eq!(downloaded.len(), 1);
        assert_eq!(downloaded[0].title, "Ep 1");

        let new_eps = db.list_episodes_by_status("new").unwrap();
        assert_eq!(new_eps.len(), 1);
    }

    // --- Status lifecycle ---

    #[test]
    fn episode_status_lifecycle() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep_id = db
            .insert_episode(p.id, "g1", "Ep", None, "https://ex.com/e.mp3", None, None)
            .unwrap();

        // new -> downloaded
        db.update_episode_audio_path(ep_id, "/tmp/audio.mp3")
            .unwrap();
        let ep = db.get_episode(ep_id).unwrap();
        assert_eq!(ep.status, EpisodeStatus::Downloaded);

        // downloaded -> transcribed
        db.update_episode_transcript_path(ep_id, "/tmp/transcript.txt")
            .unwrap();
        let ep = db.get_episode(ep_id).unwrap();
        assert_eq!(ep.status, EpisodeStatus::Transcribed);
    }

    #[test]
    fn episode_status_to_summarized() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep_id = db
            .insert_episode(p.id, "g1", "Ep", None, "https://ex.com/e.mp3", None, None)
            .unwrap();
        db.update_episode_audio_path(ep_id, "/tmp/a.mp3").unwrap();
        db.update_episode_transcript_path(ep_id, "/tmp/t.txt")
            .unwrap();
        db.insert_summary(ep_id, "Summary text", "gpt-4", Some(100), Some(50))
            .unwrap();

        let ep = db.get_episode(ep_id).unwrap();
        assert_eq!(ep.status, EpisodeStatus::Summarized);
    }

    #[test]
    fn episode_status_failed() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep_id = db
            .insert_episode(p.id, "g1", "Ep", None, "https://ex.com/e.mp3", None, None)
            .unwrap();
        db.update_episode_status(ep_id, &EpisodeStatus::Failed("download error".to_string()))
            .unwrap();

        let ep = db.get_episode(ep_id).unwrap();
        assert_eq!(
            ep.status,
            EpisodeStatus::Failed("download error".to_string())
        );
    }

    // --- Episode counts ---

    #[test]
    fn episode_count() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        assert_eq!(db.episode_count(p.id).unwrap(), 0);

        db.insert_episode(p.id, "g1", "Ep 1", None, "https://ex.com/1.mp3", None, None)
            .unwrap();
        db.insert_episode(p.id, "g2", "Ep 2", None, "https://ex.com/2.mp3", None, None)
            .unwrap();
        assert_eq!(db.episode_count(p.id).unwrap(), 2);
    }

    #[test]
    fn episode_count_by_status() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep1 = db
            .insert_episode(p.id, "g1", "Ep 1", None, "https://ex.com/1.mp3", None, None)
            .unwrap();
        db.insert_episode(p.id, "g2", "Ep 2", None, "https://ex.com/2.mp3", None, None)
            .unwrap();

        assert_eq!(db.episode_count_by_status(p.id, "new").unwrap(), 2);
        db.update_episode_audio_path(ep1, "/tmp/a.mp3").unwrap();
        assert_eq!(db.episode_count_by_status(p.id, "new").unwrap(), 1);
        assert_eq!(db.episode_count_by_status(p.id, "downloaded").unwrap(), 1);
    }

    // --- Summaries ---

    #[test]
    fn insert_and_get_summary() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep_id = db
            .insert_episode(p.id, "g1", "Ep", None, "https://ex.com/e.mp3", None, None)
            .unwrap();
        let sum_id = db
            .insert_summary(
                ep_id,
                "Great episode summary",
                "gemini-2.0-flash",
                Some(500),
                Some(200),
            )
            .unwrap();
        assert!(sum_id > 0);

        let summary = db.get_summary_by_episode(ep_id).unwrap().unwrap();
        assert_eq!(summary.content, "Great episode summary");
        assert_eq!(summary.model, "gemini-2.0-flash");
        assert_eq!(summary.prompt_tokens, Some(500));
        assert_eq!(summary.output_tokens, Some(200));
    }

    #[test]
    fn get_summary_none_when_missing() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep_id = db
            .insert_episode(p.id, "g1", "Ep", None, "https://ex.com/e.mp3", None, None)
            .unwrap();
        let summary = db.get_summary_by_episode(ep_id).unwrap();
        assert!(summary.is_none());
    }

    #[test]
    fn delete_summary_by_episode() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep_id = db
            .insert_episode(p.id, "g1", "Ep", None, "https://ex.com/e.mp3", None, None)
            .unwrap();
        db.insert_summary(ep_id, "summary", "model", None, None)
            .unwrap();
        assert!(db.get_summary_by_episode(ep_id).unwrap().is_some());

        db.delete_summary_by_episode(ep_id).unwrap();
        assert!(db.get_summary_by_episode(ep_id).unwrap().is_none());
    }

    #[test]
    fn clear_episode_transcript() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep_id = db
            .insert_episode(p.id, "g1", "Ep", None, "https://ex.com/e.mp3", None, None)
            .unwrap();
        db.update_episode_audio_path(ep_id, "/tmp/a.mp3").unwrap();
        db.update_episode_transcript_path(ep_id, "/tmp/t.txt")
            .unwrap();

        let ep = db.get_episode(ep_id).unwrap();
        assert_eq!(ep.status, EpisodeStatus::Transcribed);

        db.clear_episode_transcript(ep_id).unwrap();
        let ep = db.get_episode(ep_id).unwrap();
        assert_eq!(ep.status, EpisodeStatus::Downloaded);
        assert!(ep.transcript_path.is_none());
    }

    // --- Cascade delete ---

    #[test]
    fn cascade_delete_removes_episodes_and_summaries() {
        let db = test_db();
        let p = insert_test_podcast(&db);
        let ep_id = db
            .insert_episode(p.id, "g1", "Ep", None, "https://ex.com/e.mp3", None, None)
            .unwrap();
        db.insert_summary(ep_id, "summary", "model", None, None)
            .unwrap();

        db.delete_podcast(p.id).unwrap();

        // Episode should be gone
        assert!(db.get_episode(ep_id).is_err());
        // Summary should be gone
        assert!(db.get_summary_by_episode(ep_id).unwrap().is_none());
    }

    #[test]
    fn get_nonexistent_podcast_fails() {
        let db = test_db();
        assert!(db.get_podcast(999).is_err());
    }

    #[test]
    fn get_nonexistent_episode_fails() {
        let db = test_db();
        assert!(db.get_episode(999).is_err());
    }
}

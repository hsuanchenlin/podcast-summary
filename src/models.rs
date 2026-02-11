use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Podcast {
    pub id: i64,
    pub title: String,
    pub feed_url: String,
    pub website_url: Option<String>,
    pub description: Option<String>,
    pub last_checked: Option<DateTime<Utc>>,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: i64,
    pub podcast_id: i64,
    pub guid: String,
    pub title: String,
    pub description: Option<String>,
    pub audio_url: String,
    pub published_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub status: EpisodeStatus,
    pub audio_path: Option<String>,
    pub transcript_path: Option<String>,
    pub discovered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EpisodeStatus {
    New,
    Downloaded,
    Transcribed,
    Summarized,
    Failed(String),
}

impl EpisodeStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::New => "new",
            Self::Downloaded => "downloaded",
            Self::Transcribed => "transcribed",
            Self::Summarized => "summarized",
            Self::Failed(_) => "failed",
        }
    }

    pub fn from_db(status: &str, fail_reason: Option<&str>) -> Self {
        match status {
            "new" => Self::New,
            "downloaded" => Self::Downloaded,
            "transcribed" => Self::Transcribed,
            "summarized" => Self::Summarized,
            "failed" => Self::Failed(fail_reason.unwrap_or("unknown").to_string()),
            _ => Self::New,
        }
    }

    pub fn fail_reason(&self) -> Option<&str> {
        match self {
            Self::Failed(reason) => Some(reason),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub id: i64,
    pub episode_id: i64,
    pub content: String,
    pub model: String,
    pub prompt_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_as_str_new() {
        assert_eq!(EpisodeStatus::New.as_str(), "new");
    }

    #[test]
    fn status_as_str_downloaded() {
        assert_eq!(EpisodeStatus::Downloaded.as_str(), "downloaded");
    }

    #[test]
    fn status_as_str_transcribed() {
        assert_eq!(EpisodeStatus::Transcribed.as_str(), "transcribed");
    }

    #[test]
    fn status_as_str_summarized() {
        assert_eq!(EpisodeStatus::Summarized.as_str(), "summarized");
    }

    #[test]
    fn status_as_str_failed() {
        assert_eq!(EpisodeStatus::Failed("oops".to_string()).as_str(), "failed");
    }

    #[test]
    fn status_roundtrip_all_variants() {
        for (status_str, expected) in [
            ("new", EpisodeStatus::New),
            ("downloaded", EpisodeStatus::Downloaded),
            ("transcribed", EpisodeStatus::Transcribed),
            ("summarized", EpisodeStatus::Summarized),
        ] {
            let status = EpisodeStatus::from_db(status_str, None);
            assert_eq!(status, expected);
            assert_eq!(status.as_str(), status_str);
        }
    }

    #[test]
    fn status_failed_roundtrip() {
        let status = EpisodeStatus::from_db("failed", Some("download error"));
        assert_eq!(status, EpisodeStatus::Failed("download error".to_string()));
        assert_eq!(status.as_str(), "failed");
        assert_eq!(status.fail_reason(), Some("download error"));
    }

    #[test]
    fn status_unknown_falls_back_to_new() {
        let status = EpisodeStatus::from_db("bogus", None);
        assert_eq!(status, EpisodeStatus::New);
    }
}

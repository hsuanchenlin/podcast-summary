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

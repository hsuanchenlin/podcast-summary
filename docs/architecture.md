# Podcast Summarizer - Technical Architecture Document

## 1. Overall System Architecture and Data Flow

```
                        +-------------------+
                        |   CLI Interface   |
                        |   (clap v4.5)     |
                        +--------+----------+
                                 |
                    +------------+------------+
                    |    Command Router       |
                    +--+-----+-----+-----+---+
                       |     |     |     |
              +--------+ +---+--+ +--+--+ +--------+
              |subscribe| |sync  | |list | |summary |
              |  cmd    | | cmd  | | cmd | |  cmd   |
              +----+----+ +--+---+ +--+--+ +---+----+
                   |         |        |        |
         +---------+---------+--------+--------+---------+
         |                  Core Engine                   |
         |                                               |
         |  +----------+  +-----------+  +------------+  |
         |  |   Feed   |  | Download  |  | Transcribe |  |
         |  |  Manager |  |  Manager  |  |   Engine   |  |
         |  +----+-----+  +-----+-----+  +-----+------+  |
         |       |              |              |          |
         |  +----+-----+  +----+------+  +----+------+  |
         |  | Summarize|  |  Storage  |  |   Config  |  |
         |  |  Engine  |  |  Layer    |  |  Manager  |  |
         |  +----+-----+  +-----+-----+  +-----+-----+  |
         +-------+-------------+---------------+----------+
                 |             |               |
        +--------+--+  +------+------+  +-----+------+
        | Claude API|  |   SQLite    |  |  TOML File |
        | (HTTP)    |  |   + Files   |  |            |
        +-----------+  +-------------+  +------------+

Data Flow (sync command):
  1. CLI parses "sync" command
  2. Feed Manager fetches RSS feeds for all subscriptions (async, parallel)
  3. Feed Manager detects new episodes by comparing with DB state
  4. Download Manager streams audio files to local storage (async, parallel)
  5. Transcribe Engine converts audio -> text via whisper.cpp (local) or Whisper API
  6. Summarize Engine sends transcript to Claude API -> receives summary
  7. Storage Layer persists episode metadata, transcript path, and summary to SQLite
  8. CLI displays results to user
```

## 2. Rust Crate Recommendations

### Core Dependencies

| Category | Crate | Version | Rationale |
|---|---|---|---|
| **CLI** | `clap` | 4.5 | Industry standard, derive macros, subcommands |
| **Async Runtime** | `tokio` | 1.x | Full-featured async runtime, required by reqwest |
| **HTTP Client** | `reqwest` | 0.12 | Async HTTP, streaming downloads, TLS |
| **RSS Parsing** | `feed-rs` | 2.x | Supports RSS, Atom, JSON Feed; lightweight |
| **Database** | `rusqlite` | 0.32 | Synchronous SQLite, simple for CLI; bundled feature |
| **Serialization** | `serde` + `serde_json` | 1.x | Standard de/serialization |
| **Config** | `toml` | 0.8 | TOML parsing with serde support |
| **Audio Decode** | `symphonia` | 0.5 | Pure Rust, decodes MP3/AAC/OGG/WAV |
| **Transcription** | `whisper-rs` | 0.15 | Bindings to whisper.cpp, local inference |
| **LLM API** | `reqwest` (direct) | - | Direct HTTP calls to Claude API (simpler than wrapper crates) |
| **Error Handling** | `anyhow` + `thiserror` | 1.x | anyhow for app, thiserror for library errors |
| **Logging** | `tracing` + `tracing-subscriber` | 0.1 / 0.3 | Structured logging, async-aware |
| **Progress** | `indicatif` | 0.17 | Progress bars for downloads/transcription |
| **Date/Time** | `chrono` | 0.4 | Parsing RSS dates, episode timestamps |
| **Directories** | `dirs` | 6.x | Platform-appropriate data/config directories |

### Why These Choices

- **feed-rs over rss**: `feed-rs` handles Atom and JSON Feed in addition to RSS, so we support more podcast sources out of the box.
- **rusqlite over sqlx**: For a CLI tool, synchronous SQLite is simpler. No need for async DB access when queries are fast and local. Avoids compile-time query checking overhead.
- **whisper-rs for local transcription**: Keeps audio data local, no API costs for transcription, GPU acceleration available. Falls back to CPU.
- **Direct reqwest for Claude API**: The Anthropic Rust SDKs are mostly community-maintained. Direct HTTP with reqwest + serde is more stable and gives us full control over request/response handling.
- **symphonia for audio**: Pure Rust, no FFmpeg dependency, handles the common podcast audio formats (MP3, AAC, OGG).

## 3. Module / File Structure

```
podcast_summarize/
├── Cargo.toml
├── Cargo.lock
├── docs/
│   └── architecture.md
├── src/
│   ├── main.rs                 # Entry point, CLI setup
│   ├── cli.rs                  # clap command definitions and argument parsing
│   ├── config.rs               # Config file loading/saving (TOML)
│   ├── db.rs                   # SQLite schema, migrations, queries
│   ├── models.rs               # Core data structs (Podcast, Episode, Summary)
│   ├── error.rs                # Error types (thiserror) + Result alias
│   ├── feed.rs                 # RSS feed fetching and parsing
│   ├── download.rs             # Audio file download (streaming)
│   ├── audio.rs                # Audio format detection and WAV conversion
│   ├── transcribe.rs           # Whisper transcription (local whisper.cpp)
│   ├── summarize.rs            # Claude API integration for summaries
│   └── commands/
│       ├── mod.rs
│       ├── subscribe.rs        # Add/remove/list subscriptions
│       ├── sync.rs             # Fetch feeds, download, transcribe, summarize
│       ├── list.rs             # List episodes, summaries
│       └── show.rs             # Show a specific episode's summary/transcript
└── tests/
    ├── integration/
    │   ├── feed_test.rs
    │   ├── db_test.rs
    │   └── pipeline_test.rs
    └── fixtures/
        ├── sample_rss.xml
        └── sample_audio.wav
```

### Cargo.toml

```toml
[package]
name = "podcast-summarize"
version = "0.1.0"
edition = "2024"
description = "CLI tool to subscribe to podcasts, transcribe episodes, and generate summaries"

[dependencies]
# CLI
clap = { version = "4.5", features = ["derive"] }

# Async
tokio = { version = "1", features = ["full"] }

# HTTP
reqwest = { version = "0.12", features = ["stream", "json"] }

# RSS
feed-rs = "2"

# Database
rusqlite = { version = "0.32", features = ["bundled"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Audio
symphonia = { version = "0.5", features = ["mp3", "aac", "ogg", "wav", "pcm"] }

# Transcription (local whisper.cpp)
whisper-rs = "0.15"

# Error handling
anyhow = "1"
thiserror = "2"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Progress bars
indicatif = "0.17"

# Date/time
chrono = { version = "0.4", features = ["serde"] }

# Directories
dirs = "6"

# Bytes for streaming
futures-util = "0.3"
```

## 4. Data Models

```rust
// --- src/models.rs ---

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A podcast subscription
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

/// A single episode from a podcast
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: i64,
    pub podcast_id: i64,
    pub guid: String,                        // RSS guid, unique per episode
    pub title: String,
    pub description: Option<String>,
    pub audio_url: String,
    pub published_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub status: EpisodeStatus,
    pub audio_path: Option<String>,          // Local path to downloaded audio
    pub transcript_path: Option<String>,     // Local path to transcript text
    pub discovered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EpisodeStatus {
    New,            // Discovered, not yet downloaded
    Downloaded,     // Audio file downloaded
    Transcribed,    // Transcript generated
    Summarized,     // Summary generated
    Failed(String), // Processing failed with reason
}

/// A generated summary for an episode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub id: i64,
    pub episode_id: i64,
    pub content: String,             // The summary text
    pub model: String,               // e.g. "claude-sonnet-4-5-20250929"
    pub prompt_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub created_at: DateTime<Utc>,
}

// --- src/config.rs ---

/// Application configuration (stored as TOML)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub transcription: TranscriptionConfig,
    pub summarization: SummarizationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub data_dir: Option<String>,        // Override default data directory
    pub max_concurrent_downloads: usize, // Default: 3
    pub auto_transcribe: bool,           // Transcribe on download. Default: true
    pub auto_summarize: bool,            // Summarize on transcription. Default: true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    pub backend: TranscriptionBackend,
    pub whisper_model: String,           // "base", "small", "medium", "large"
    pub whisper_model_path: Option<String>, // Custom model file path
    pub language: Option<String>,        // e.g. "en", None for auto-detect
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranscriptionBackend {
    Local,    // whisper.cpp via whisper-rs
    Api,      // OpenAI Whisper API
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizationConfig {
    pub api_key_env: String,             // Env var name for API key. Default: "ANTHROPIC_API_KEY"
    pub model: String,                   // Default: "claude-sonnet-4-5-20250929"
    pub max_tokens: u32,                 // Default: 4096
    pub system_prompt: Option<String>,   // Custom system prompt for summaries
}
```

## 5. API Integration Strategy

### Transcription: Local whisper.cpp (Primary) + Whisper API (Fallback)

**Primary: Local via whisper-rs**
```
Audio File -> symphonia (decode to PCM/WAV) -> whisper-rs (whisper.cpp) -> Text
```

- whisper-rs wraps whisper.cpp and runs locally
- Models downloaded on first use to `~/.local/share/podcast-summarize/models/`
- Supports CPU and GPU (CUDA, Metal on macOS)
- Model sizes: base (~142MB), small (~466MB), medium (~1.5GB), large (~2.9GB)
- Default to "base" model for speed; user can configure larger models
- Audio must be 16kHz mono WAV; symphonia handles conversion from MP3/AAC/OGG

**Fallback: OpenAI Whisper API**
- For users who don't want local model overhead
- Requires `OPENAI_API_KEY` env var
- POST multipart to `https://api.openai.com/v1/audio/transcriptions`
- 25MB file size limit per request; chunk longer episodes

**Trade-offs:**
| Factor | Local (whisper.cpp) | Cloud (Whisper API) |
|---|---|---|
| Cost | Free (after model download) | ~$0.006/min |
| Speed | Depends on hardware (GPU fast) | Consistent, fast |
| Privacy | Audio stays local | Audio sent to OpenAI |
| Setup | Download model (~142MB-2.9GB) | Just API key |
| Quality | Same Whisper models | Same Whisper models |
| Offline | Yes | No |

Recommendation: Default to **local** for privacy and zero ongoing cost. Podcast audio is not sensitive in most cases, but local processing avoids rate limits and costs.

### Summarization: Claude API

```
Transcript Text -> Chunk if needed -> Claude Messages API -> Summary
```

- Direct HTTP via reqwest to `https://api.anthropic.com/v1/messages`
- Authentication via `ANTHROPIC_API_KEY` env var
- Default model: `claude-sonnet-4-5-20250929` (good balance of speed/quality)
- System prompt crafted for podcast summarization:
  ```
  You are a podcast summarizer. Given a transcript of a podcast episode,
  produce a structured summary with: key topics discussed, main takeaways,
  notable quotes, and a brief overall summary. Be concise but comprehensive.
  ```
- Handle long transcripts by chunking (Claude supports 200K tokens, most episodes fit)
- For very long episodes (>3 hours): split transcript into segments, summarize each, then produce a final combined summary
- Parse response for token usage tracking (stored in Summary model)

**Request format:**
```rust
#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,    // "user"
    content: String, // The transcript
}
```

## 6. Storage Strategy

### Directory Layout
```
~/.local/share/podcast-summarize/     (Linux)
~/Library/Application Support/podcast-summarize/  (macOS)
├── db.sqlite3                # Main database
├── audio/                    # Downloaded audio files
│   └── {podcast_id}/
│       └── {episode_guid_hash}.mp3
├── transcripts/              # Generated transcripts
│   └── {podcast_id}/
│       └── {episode_guid_hash}.txt
└── models/                   # Whisper model files
    └── ggml-base.bin

~/.config/podcast-summarize/          (Linux)
~/Library/Application Support/podcast-summarize/  (macOS)
└── config.toml               # User configuration
```

### SQLite Schema

SQLite is chosen over JSON files because:
- ACID transactions prevent corruption from interrupted syncs
- Efficient querying (list episodes by podcast, filter by status)
- Single file, easy to back up
- No serialization overhead for partial reads

```sql
CREATE TABLE podcasts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    title       TEXT NOT NULL,
    feed_url    TEXT NOT NULL UNIQUE,
    website_url TEXT,
    description TEXT,
    last_checked TEXT,  -- ISO 8601
    added_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE episodes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    podcast_id      INTEGER NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE,
    guid            TEXT NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT,
    audio_url       TEXT NOT NULL,
    published_at    TEXT,         -- ISO 8601
    duration_secs   INTEGER,
    status          TEXT NOT NULL DEFAULT 'new',
    audio_path      TEXT,
    transcript_path TEXT,
    discovered_at   TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(podcast_id, guid)
);

CREATE TABLE summaries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    episode_id    INTEGER NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
    content       TEXT NOT NULL,
    model         TEXT NOT NULL,
    prompt_tokens INTEGER,
    output_tokens INTEGER,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_episodes_podcast_id ON episodes(podcast_id);
CREATE INDEX idx_episodes_status ON episodes(status);
CREATE INDEX idx_summaries_episode_id ON summaries(episode_id);
```

### Storage Decisions

| Data | Storage | Reason |
|---|---|---|
| Subscriptions | SQLite | Relational, queryable |
| Episode metadata | SQLite | Relational, status tracking |
| Summaries | SQLite | Queryable, small text |
| Audio files | Filesystem | Large binary blobs, streamed |
| Transcripts | Filesystem | Large text, may want grep/search |
| Whisper models | Filesystem | Large binary, shared across episodes |
| Config | TOML file | Human-editable, version-controllable |

Audio files can optionally be deleted after transcription to save disk space (configurable).

## 7. Error Handling Strategy

### Layered Approach

**Library-level errors with `thiserror`** (src/error.rs):
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Feed parsing failed for {url}: {source}")]
    FeedParse { url: String, source: feed_rs::parser::ParseFeedError },

    #[error("Audio decoding failed: {0}")]
    AudioDecode(String),

    #[error("Transcription failed: {0}")]
    Transcription(String),

    #[error("Claude API error ({status}): {body}")]
    ClaudeApi { status: u16, body: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Episode not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AppError>;
```

**Application-level with `anyhow`** in command handlers:
- Commands use `anyhow::Result` for convenience with `.context()` for adding error context
- Library modules use the typed `AppError` for programmatic error handling

**Strategy:**
- Network errors (feed fetch, download, API calls): Retry up to 3 times with exponential backoff
- Per-episode errors: Log and continue, don't abort the entire sync. Mark episode as `Failed(reason)`
- Fatal errors (DB corruption, missing config): Exit with clear error message
- User-facing errors: Formatted nicely with suggestions (e.g., "API key not set. Run `podcast-summarize config set api-key` or set ANTHROPIC_API_KEY")

## 8. Concurrency Approach

### Async Runtime: Tokio

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // CLI parsing, dispatch to commands
}
```

### Parallel Operations

**Feed Fetching** (fully parallel):
```rust
use futures_util::future::join_all;

let feeds: Vec<_> = podcasts.iter()
    .map(|p| fetch_feed(&client, &p.feed_url))
    .collect();
let results = join_all(feeds).await;
```

**Audio Downloads** (bounded concurrency):
```rust
use tokio::sync::Semaphore;

let semaphore = Arc::new(Semaphore::new(config.max_concurrent_downloads)); // default 3
for episode in new_episodes {
    let permit = semaphore.clone().acquire_owned().await?;
    tokio::spawn(async move {
        download_episode(&client, &episode, &audio_dir).await;
        drop(permit);
    });
}
```

**Transcription** (sequential or limited parallel):
- whisper.cpp is CPU/GPU intensive
- Default: process one episode at a time
- If GPU available, can potentially run 1-2 in parallel

**Summarization** (bounded parallel):
- Claude API has rate limits
- Use semaphore with limit of 2-3 concurrent requests
- Respect rate limit headers in responses

### Pipeline Architecture

The sync command runs a pipeline per episode:
```
Feed Fetch -> Download -> Transcribe -> Summarize
```

Episodes are independent, so the pipeline can process multiple episodes concurrently. However, within a single episode, the stages are sequential (can't transcribe before downloading).

```rust
// Simplified pipeline
async fn process_episode(episode: &Episode) -> Result<()> {
    let audio_path = download::fetch_audio(&episode.audio_url).await?;
    // Transcription uses blocking whisper-rs; run in spawn_blocking
    let transcript = tokio::task::spawn_blocking(move || {
        transcribe::transcribe_audio(&audio_path, &whisper_config)
    }).await??;
    let summary = summarize::generate_summary(&transcript).await?;
    db::save_results(episode.id, &transcript, &summary)?;
    Ok(())
}
```

Note: `whisper-rs` is synchronous (C bindings). Wrap in `tokio::task::spawn_blocking` to avoid blocking the async runtime.

### SQLite Concurrency

- rusqlite is synchronous; wrap DB calls in `spawn_blocking` if called from async context
- Use a single connection with WAL mode for concurrent reads
- Alternatively, use a connection pool via `r2d2-rusqlite` if needed
- For this CLI tool, a single connection is likely sufficient

---

## CLI Interface Design

```
podcast-summarize subscribe <feed_url>     # Add a podcast subscription
podcast-summarize unsubscribe <feed_url>   # Remove a subscription
podcast-summarize list podcasts             # List all subscriptions
podcast-summarize list episodes [--podcast <name>] [--status <status>]
podcast-summarize sync [--podcast <name>]   # Fetch, download, transcribe, summarize
podcast-summarize show <episode_id>         # Show episode summary and details
podcast-summarize config set <key> <value>  # Update configuration
podcast-summarize config show               # Show current configuration
```

---

## Summary of Key Architectural Decisions

1. **Local-first transcription** with whisper.cpp via whisper-rs -- zero cost, offline capable, privacy preserving
2. **SQLite for state** -- reliable, single-file, ACID, perfect for CLI tools
3. **Direct HTTP for Claude API** -- no dependency on community wrapper crates that may lag behind API changes
4. **Tokio async** for I/O-bound work (network), `spawn_blocking` for CPU-bound work (transcription)
5. **symphonia for audio** -- pure Rust, no FFmpeg dependency, handles common podcast formats
6. **feed-rs for RSS** -- broader format support than the `rss` crate alone
7. **Per-episode error isolation** -- one failing episode doesn't block others
8. **Configurable pipeline** -- users can disable auto-transcribe or auto-summarize

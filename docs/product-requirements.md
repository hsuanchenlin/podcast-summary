# Product Requirements Document: `podsummary` - Podcast Summarizer CLI

## 1. Overview

`podsummary` is a Rust command-line tool that lets users subscribe to podcasts via RSS feeds, automatically download new episodes, and generate AI-powered summaries. It targets power users and developers who follow multiple podcasts and want quick, scannable summaries without listening to full episodes.

## 2. User Personas

### Primary: Busy Developer / Knowledge Worker
- Follows 5-20 podcasts across tech, business, science
- Doesn't have time to listen to all episodes
- Wants to triage episodes: read summary first, listen only if interesting
- Comfortable with CLI tools, config files, and API keys

### Secondary: Researcher / Journalist
- Needs to monitor specific podcasts for topics relevant to their work
- Wants searchable, archivable summaries
- Values accuracy and attribution

---

## 3. User Stories and Acceptance Criteria

### 3.1 MVP (v0.1) User Stories

#### US-1: Subscribe to a podcast by RSS feed URL
**As a** user, **I want to** add a podcast by providing its RSS feed URL **so that** I can track new episodes.

**Acceptance Criteria:**
- `podsummary add <RSS_URL>` validates the feed and stores subscription
- Feed metadata (title, author, description) is displayed on success
- Duplicate feeds are rejected with a clear message
- Invalid URLs or non-RSS content produce helpful error messages

#### US-2: List subscribed podcasts
**As a** user, **I want to** see all my subscribed podcasts **so that** I can manage my subscriptions.

**Acceptance Criteria:**
- `podsummary list` shows all subscriptions with title, episode count, last updated
- `podsummary list <name>` shows episodes for a specific podcast
- Supports partial name matching (case-insensitive)

#### US-3: Fetch new episodes
**As a** user, **I want to** check for and download new episodes **so that** I have content to summarize.

**Acceptance Criteria:**
- `podsummary fetch` checks all subscribed feeds for new episodes
- `podsummary fetch <name>` checks a specific podcast
- Only downloads episodes not already processed
- Shows progress (download speed, file size, ETA)
- Respects configurable download directory

#### US-4: Summarize an episode
**As a** user, **I want to** generate an AI summary of a podcast episode **so that** I can quickly understand its content.

**Acceptance Criteria:**
- `podsummary summarize <name> [episode]` generates a summary
- Summary includes: title, date, duration, key topics, main takeaways, notable quotes
- Summary is displayed in the terminal with readable formatting
- Summary is cached locally so repeated calls don't re-process
- If no episode is specified, summarizes the latest unread episode

#### US-5: Read summaries
**As a** user, **I want to** read previously generated summaries **so that** I can review them later.

**Acceptance Criteria:**
- `podsummary read <name> [episode]` displays a cached summary
- `podsummary read --unread` shows all unread summaries
- Summaries are marked as read after display

#### US-6: Remove a subscription
**As a** user, **I want to** unsubscribe from a podcast **so that** I stop tracking it.

**Acceptance Criteria:**
- `podsummary remove <name>` removes the subscription
- Prompts for confirmation (unless `--yes` flag is passed)
- Optionally deletes cached episodes and summaries with `--purge`

#### US-7: Initialize configuration
**As a** user, **I want to** set up my configuration (API keys, preferences) **so that** the tool works correctly.

**Acceptance Criteria:**
- `podsummary init` creates a default config file interactively
- Prompts for required settings (AI provider, API key, download directory)
- Validates API key connectivity before saving
- Config file location follows XDG conventions (`~/.config/podsummary/config.toml`)

### 3.2 Nice-to-Have (v0.2+) User Stories

#### US-8: Search for podcasts by name
**As a** user, **I want to** search for podcasts by name **so that** I don't need to find the RSS URL myself.

**Acceptance Criteria:**
- `podsummary search <query>` searches a podcast directory (e.g., iTunes/Apple Podcasts API or PodcastIndex)
- Displays results with title, author, description, feed URL
- Interactive prompt to subscribe from search results

#### US-9: Automatic scheduled fetching
**As a** user, **I want** episodes to be fetched automatically on a schedule **so that** summaries are ready when I check.

**Acceptance Criteria:**
- `podsummary daemon` runs a background process that checks feeds on an interval
- Configurable interval per podcast or globally
- Optionally auto-summarizes new episodes

#### US-10: Export summaries
**As a** user, **I want to** export summaries in different formats **so that** I can share or archive them.

**Acceptance Criteria:**
- `podsummary export <name> --format <md|json|txt>` exports summaries
- Supports exporting all summaries for a podcast or a single episode

#### US-11: Filter and tag episodes
**As a** user, **I want to** filter episodes by topic or date **so that** I can find relevant content quickly.

**Acceptance Criteria:**
- `podsummary read --since <date>` filters by date
- `podsummary read --topic <keyword>` filters by AI-detected topics

#### US-12: Multiple AI provider support
**As a** user, **I want to** choose my AI provider **so that** I can use whichever service I prefer or have access to.

**Acceptance Criteria:**
- Supports OpenAI, Anthropic Claude, and local models (via Ollama)
- Configurable in config file per-podcast or globally

---

## 4. Feature Prioritization

### MVP (v0.1) - Core Loop
| Priority | Feature | User Story |
|----------|---------|------------|
| P0 | Subscribe by RSS URL | US-1 |
| P0 | List subscriptions & episodes | US-2 |
| P0 | Fetch/download episodes | US-3 |
| P0 | Summarize episodes (single AI provider) | US-4 |
| P0 | Read cached summaries | US-5 |
| P1 | Remove subscriptions | US-6 |
| P1 | Interactive config init | US-7 |

### Post-MVP (v0.2+)
| Priority | Feature | User Story |
|----------|---------|------------|
| P2 | Search podcasts by name | US-8 |
| P2 | Export summaries | US-10 |
| P2 | Multiple AI providers | US-12 |
| P3 | Background daemon | US-9 |
| P3 | Topic filtering | US-11 |

---

## 5. CLI UX Design

### 5.1 Command Structure

```
podsummary <COMMAND> [OPTIONS]

COMMANDS:
  init                    Initialize configuration interactively
  add <URL>               Subscribe to a podcast by RSS feed URL
  remove <NAME>           Unsubscribe from a podcast
  list [NAME]             List subscriptions or episodes
  fetch [NAME]            Download new episodes
  summarize <NAME> [EP]   Generate AI summary for an episode
  read [NAME] [EP]        Display cached summaries
  config                  Show or edit configuration
  help                    Show help information

GLOBAL OPTIONS:
  -c, --config <PATH>     Path to config file
  -v, --verbose           Enable verbose output
  -q, --quiet             Suppress non-essential output
  --no-color              Disable colored output
  --json                  Output in JSON format (for scripting)
  -h, --help              Print help
  -V, --version           Print version
```

### 5.2 Command Details and Examples

#### `podsummary add`
```
$ podsummary add https://feeds.example.com/techpod.xml

  Added: Tech Deep Dive
  Author: Jane Smith
  Episodes: 142
  Latest: "AI in 2026" (2026-02-08)
  Feed URL: https://feeds.example.com/techpod.xml

$ podsummary add https://feeds.example.com/techpod.xml
  Error: Already subscribed to "Tech Deep Dive"
```

#### `podsummary list`
```
$ podsummary list

  PODCAST              EPISODES  UNREAD  LAST UPDATED
  Tech Deep Dive       142       3       2026-02-08
  The Changelog        580       1       2026-02-07
  Syntax FM            450       5       2026-02-06

$ podsummary list "tech deep"

  Tech Deep Dive (142 episodes)
  ─────────────────────────────
  #142  AI in 2026                        2026-02-08  45m  [new]
  #141  Rust vs Go in Production          2026-02-01  38m  [new]
  #140  Database Scaling Wars             2026-01-25  52m  [new]
  #139  WebAssembly Beyond the Browser    2026-01-18  41m  [read]
  #138  The State of DevOps               2026-01-11  35m  [read]
  ...
```

#### `podsummary fetch`
```
$ podsummary fetch

  Checking feeds...
  Tech Deep Dive: 2 new episodes
    Downloading "AI in 2026" [████████████████████] 45.2 MB  done
    Downloading "Rust vs Go"  [████████████████████] 38.1 MB  done
  The Changelog: 1 new episode
    Downloading "State of JS" [████████░░░░░░░░░░░░] 12.3/28.4 MB  43%
  Syntax FM: up to date

  Done. 3 new episodes downloaded.
```

#### `podsummary summarize`
```
$ podsummary summarize "tech deep" 142

  Summarizing "AI in 2026" (Tech Deep Dive #142)...
  Transcribing audio... done (4m 12s)
  Generating summary... done (8s)

  ═══════════════════════════════════════════════════
  AI in 2026 - Tech Deep Dive #142
  Published: 2026-02-08 | Duration: 45 min
  ═══════════════════════════════════════════════════

  TOPICS: artificial intelligence, LLMs, AI agents, regulation

  SUMMARY:
  Jane Smith interviews Dr. Alex Chen about the current state of
  AI in 2026. The discussion covers three main areas: the maturation
  of AI agent frameworks, new EU regulations on AI transparency,
  and the emerging trend of on-device LLMs replacing cloud APIs
  for privacy-sensitive applications.

  KEY TAKEAWAYS:
  - On-device models now handle 80% of tasks that required cloud
    APIs two years ago
  - EU AI Act enforcement has driven a shift toward explainable AI
  - Agent-to-agent protocols are becoming standardized
  - Small businesses are the fastest-growing AI adopters

  NOTABLE QUOTES:
  - "The real revolution isn't bigger models, it's smaller models
     that actually work." - Dr. Alex Chen (12:34)
  - "Regulation didn't kill innovation; it redirected it." - Jane
     Smith (28:15)

  ═══════════════════════════════════════════════════
```

#### `podsummary read`
```
$ podsummary read --unread

  3 unread summaries:

  [1] Tech Deep Dive #142 - AI in 2026 (2026-02-08)
  [2] Tech Deep Dive #141 - Rust vs Go in Production (2026-02-01)
  [3] The Changelog #580 - State of JS (2026-02-07)

  Select episode to read (1-3, or 'a' for all): 1
  ...displays summary...
```

### 5.3 Output Format

- Default: human-readable, colored terminal output with Unicode box-drawing
- `--json`: machine-readable JSON for scripting/piping
- `--quiet`: minimal output (just results, no progress)
- `--no-color`: plain text (for piping to files or non-color terminals)

---

## 6. Configuration File Format

Location: `~/.config/podsummary/config.toml` (XDG-compliant)

```toml
# podsummary configuration

[general]
# Directory for downloaded episodes and cached data
data_dir = "~/.local/share/podsummary"
# Maximum concurrent downloads
max_concurrent_downloads = 3
# Auto-cleanup downloaded audio after summarization
auto_cleanup_audio = true
# Default output format: "text", "json"
default_format = "text"

[ai]
# AI provider: "openai", "anthropic", "ollama"
provider = "openai"
# API key (can also use env var PODSUMMARY_API_KEY)
api_key = "sk-..."
# Model to use for summarization
model = "gpt-4o"
# Model to use for transcription (if using cloud transcription)
transcription_model = "whisper-1"
# Summary language
language = "en"

[ai.summary]
# Summary style: "brief", "detailed", "bullet-points"
style = "detailed"
# Include notable quotes
include_quotes = true
# Include topic tags
include_topics = true
# Maximum summary length (approximate word count)
max_length = 500

[transcription]
# Transcription backend: "local-whisper", "openai-whisper", "cloud"
backend = "local-whisper"
# Path to local whisper model (if using local-whisper)
# model_path = "/path/to/whisper/model"
# Whisper model size: "tiny", "base", "small", "medium", "large"
whisper_model_size = "base"

[fetch]
# How often to check for new episodes (in hours), used by daemon mode
check_interval_hours = 6
# Maximum episode age to fetch (in days), 0 = no limit
max_episode_age_days = 30
# Download episodes automatically on fetch, or just update feed metadata
auto_download = true

# Per-podcast overrides use [[podcast]] sections
# These are managed by `podsummary add` but can be edited manually

[[podcast]]
name = "Tech Deep Dive"
url = "https://feeds.example.com/techpod.xml"
# Override global settings per podcast:
# auto_download = false
# ai.summary.style = "brief"
```

### 6.1 Configuration Precedence (highest to lowest)

1. CLI flags (`--model`, `--style`, etc.)
2. Environment variables (`PODSUMMARY_API_KEY`, `PODSUMMARY_DATA_DIR`)
3. Per-podcast overrides in config file
4. Global config file settings
5. Built-in defaults

### 6.2 Environment Variables

| Variable | Description |
|----------|-------------|
| `PODSUMMARY_API_KEY` | AI provider API key (overrides config) |
| `PODSUMMARY_CONFIG` | Custom config file path |
| `PODSUMMARY_DATA_DIR` | Custom data directory |
| `NO_COLOR` | Disable colored output (standard) |

---

## 7. Data Storage

### 7.1 Directory Structure

```
~/.local/share/podsummary/
  db.json                          # Subscription metadata & state
  cache/
    <podcast-slug>/
      feed.xml                     # Cached RSS feed
      episodes/
        <episode-slug>.mp3         # Downloaded audio
      summaries/
        <episode-slug>.json        # Generated summary
      transcripts/
        <episode-slug>.txt         # Transcription cache
```

### 7.2 State Tracking

The `db.json` file tracks:
- Subscribed feeds with metadata
- Per-episode state: `new` | `downloaded` | `transcribed` | `summarized` | `read`
- Last fetch timestamp per feed
- Summary generation metadata (model used, timestamp)

---

## 8. Summarization Pipeline

The core workflow for generating a summary:

```
RSS Feed -> Download Audio -> Transcribe -> Summarize -> Cache & Display
```

### 8.1 Step Details

1. **Download Audio**: Fetch the MP3/audio enclosure from the RSS feed item
2. **Transcribe**: Convert audio to text
   - Preferred: Local Whisper (via `whisper-rs` bindings) for privacy and cost
   - Alternative: OpenAI Whisper API for better accuracy on large models
3. **Summarize**: Send transcript to LLM with structured prompt
   - Prompt requests: summary, key takeaways, topics, notable quotes
   - Response is parsed into a structured format
4. **Cache**: Store transcript and summary as local files
5. **Display**: Render summary in terminal with formatting

### 8.2 Chunking Strategy

For long episodes (>1 hour), transcripts may exceed LLM context windows:
- Split transcript into overlapping chunks (~10k tokens each, 500 token overlap)
- Summarize each chunk independently
- Generate a final consolidated summary from chunk summaries

---

## 9. Edge Cases and Error Scenarios

### 9.1 Network & Feed Errors
| Scenario | Handling |
|----------|----------|
| RSS feed URL returns 404 | Error message with suggestion to verify URL |
| Feed URL redirects | Follow redirects (up to 5), update stored URL |
| Feed XML is malformed | Attempt lenient parsing; if fails, report specific XML error |
| Feed has no audio enclosures | Warn that feed has no downloadable episodes |
| Network timeout during download | Retry with exponential backoff (3 attempts) |
| Partial download (interrupted) | Resume download if server supports Range headers |
| Feed requires authentication | Support basic auth via config (`url_username`, `url_password`) |

### 9.2 Audio & Transcription Errors
| Scenario | Handling |
|----------|----------|
| Audio format not MP3 (M4A, OGG, etc.) | Support common formats; error on unsupported with suggestion |
| Very long episodes (>3 hours) | Warn about processing time; proceed with chunked approach |
| Audio is music/non-speech | Detect low word count in transcript; warn user |
| Whisper model not found locally | Prompt to download; provide command to install |
| Transcription produces garbage | Detect via heuristics (repetition, low confidence); warn user |

### 9.3 AI/Summarization Errors
| Scenario | Handling |
|----------|----------|
| API key invalid or expired | Clear error message with instructions to update config |
| API rate limit hit | Retry with backoff; show remaining wait time |
| API returns error | Show error detail; suggest retry or different model |
| Context window exceeded | Automatic chunking (see 8.2) |
| Summary quality is poor | Allow re-summarize with `--force` flag |

### 9.4 Configuration & State Errors
| Scenario | Handling |
|----------|----------|
| Config file missing | Run `podsummary init` suggestion; use defaults where possible |
| Config file has invalid TOML | Parse error with line number and suggestion |
| Data directory not writable | Error with suggestion to check permissions or change `data_dir` |
| Disk space low | Check before download; warn if <500MB available |
| Database corruption | Backup db.json before writes; provide `podsummary repair` hint |

### 9.5 Concurrency & Edge Cases
| Scenario | Handling |
|----------|----------|
| Multiple instances running | File lock on db.json to prevent corruption |
| Ctrl+C during operation | Graceful shutdown; clean up partial files |
| Episode deleted from feed | Keep local data; mark as "removed from feed" |
| Feed changes URL for existing episode | Match by GUID, not URL |
| Unicode/non-ASCII podcast names | Full UTF-8 support; slugify for filesystem paths |

---

## 10. Non-Functional Requirements

### 10.1 Performance
- Feed checking should complete in <5s per feed (network permitting)
- Summary display from cache should be instant (<100ms)
- Transcription of a 1-hour episode: <5 min with local Whisper base model
- Concurrent downloads should not exceed configurable limit

### 10.2 Reliability
- All state changes are atomic (write temp file, then rename)
- Graceful degradation: if AI provider is down, all non-summary features still work
- Idempotent operations: running `fetch` or `summarize` twice produces the same result

### 10.3 Security
- API keys stored in config file with 600 permissions
- Support for environment variable API keys (preferred for shared machines)
- No telemetry or external data sharing
- Local-first: all data stays on disk unless explicitly using cloud APIs

### 10.4 Compatibility
- Rust edition 2021+, MSRV 1.75+
- Platforms: Linux (primary), macOS, Windows (best-effort)
- Terminal: any terminal supporting UTF-8; graceful fallback for no-color

---

## 11. Competitive Analysis

### Existing Tools (Gaps We Fill)

| Tool | Language | Summarization | CLI | Notes |
|------|----------|--------------|-----|-------|
| castget | C | No | Yes | Download only, no transcription or AI |
| castero | Python | No | TUI | Player-focused, no summarization |
| podcast (crate) | Rust | No | Yes | Subscribe + play, no AI features |
| podclaw | Rust | No | Yes | Feed management + download only |
| podcast-summarizer | Python | Yes | No | Web UI (Streamlit), not CLI |
| Snipd | N/A | Yes | No | Mobile app, proprietary |
| NoteGPT | N/A | Yes | No | Web-only, proprietary |

**Our differentiation**: The only CLI-native tool combining RSS management with AI-powered transcription and summarization, written in Rust for performance and reliability, with local-first design and configurable AI backends.

---

## 12. Success Metrics (Personal Tool)

Since this is a personal productivity tool, success is measured by:
- **Daily usage**: Do I actually check summaries before deciding to listen?
- **Time saved**: Am I able to triage 10+ episodes/week in <10 minutes of reading?
- **Summary quality**: Are summaries accurate enough that I rarely miss important content?
- **Reliability**: Does it "just work" when I run it?

---

## 13. Open Questions

1. **Transcript storage**: Should we store full transcripts by default, or only summaries? (Disk space vs. searchability tradeoff)
2. **Podcast search API**: iTunes Search API vs. PodcastIndex API? PodcastIndex is open-source-friendly.
3. **Local whisper vs. cloud**: Should MVP default to local whisper (slower, free) or cloud API (faster, paid)?
4. **Episode selection UX**: Episode numbers from the RSS feed, or our own sequential numbering?
5. **OPML import/export**: Should we support OPML for importing subscriptions from other podcast apps?

# podcast-summarize

A Rust CLI tool that subscribes to podcasts via RSS, downloads episodes, transcribes audio locally with whisper.cpp, and generates AI summaries.

## Features

- Subscribe to any podcast via RSS feed URL
- Download episodes with concurrent downloads
- Local transcription using whisper.cpp (no cloud API needed)
- AI-powered summaries via OpenAI-compatible APIs (Gemini, OpenAI, DeepSeek, etc.)
- Traditional Chinese support with customizable initial prompts
- Configurable CPU usage for transcription
- SQLite database for tracking episodes and summaries

## Install

```bash
cargo build --release
cp target/release/podcast-summarize ~/.local/bin/
```

Requires [CMake](https://cmake.org/) for building whisper.cpp:

```bash
brew install cmake  # macOS
```

## Quick Start

```bash
# Subscribe to a podcast
podcast-summarize add https://example.com/feed.xml

# List subscribed podcasts
podcast-summarize list

# List episodes for a podcast (by ID or name)
podcast-summarize list 1
podcast-summarize list "podcast name"

# Sync all podcasts (download + transcribe + summarize)
podcast-summarize sync

# Sync a specific podcast
podcast-summarize sync 1

# Sync a specific episode
podcast-summarize sync -e 42

# Download only
podcast-summarize sync --download-only

# Re-transcribe with a different model
podcast-summarize sync -e 42 --redo

# Limit CPU usage during transcription
podcast-summarize sync -e 42 --cpu 50

# Read a summary
podcast-summarize show 42

# Read the transcript
podcast-summarize show 42 -t

# Remove a subscription
podcast-summarize remove "podcast name"
```

## Configuration

Config file: `~/Library/Application Support/podcast-summarize/config.toml` (macOS)

```toml
[general]
auto_cleanup_audio = false

[transcription]
language = "zh"
whisper_model = "large-v3"    # tiny, base, small, medium, large-v3
cpu_percent = 80              # 1-100
initial_prompt = "以下是繁體中文的Podcast逐字稿。"

[summarization]
api_base_url = "https://generativelanguage.googleapis.com/v1beta/openai"
api_key_env = "GEMINI_API_KEY"
model = "gemini-2.0-flash"
max_tokens = 4096
```

Set config from CLI:

```bash
podcast-summarize config set cpu_percent 50
podcast-summarize config set whisper_model large-v3
podcast-summarize config set language zh
podcast-summarize config show
```

### Supported API Providers

Any OpenAI-compatible chat completions API works:

| Provider | `api_base_url` | `api_key_env` |
|----------|---------------|---------------|
| Gemini | `https://generativelanguage.googleapis.com/v1beta/openai` | `GEMINI_API_KEY` |
| OpenAI | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| DeepSeek | `https://api.deepseek.com/v1` | `DEEPSEEK_API_KEY` |

## Data Storage

All data is stored in `~/Library/Application Support/podcast-summarize/`:

```
├── db.sqlite3          # episode metadata and summaries
├── audio/              # downloaded audio files
├── transcripts/        # transcription text files
├── models/             # whisper model files
└── config.toml         # configuration
```

## License

MIT

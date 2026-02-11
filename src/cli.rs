use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "podcast-summarize")]
#[command(about = "Subscribe to podcasts, transcribe episodes, and generate AI summaries")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Subscribe to a podcast by RSS feed URL
    Add {
        /// RSS feed URL
        url: String,
    },

    /// Remove a podcast subscription
    Remove {
        /// Podcast name (partial match)
        name: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,

        /// Also delete downloaded files and summaries
        #[arg(long)]
        purge: bool,
    },

    /// List subscriptions or episodes
    List {
        /// Podcast name to show episodes for (partial match)
        name: Option<String>,
    },

    /// Fetch new episodes, download, transcribe, and summarize
    Sync {
        /// Only sync a specific podcast (partial match)
        name: Option<String>,

        /// Process a specific episode by ID
        #[arg(short, long)]
        episode: Option<i64>,

        /// Skip transcription and summarization (download only)
        #[arg(long)]
        download_only: bool,

        /// Force re-transcribe and re-summarize (delete old results)
        #[arg(long)]
        redo: bool,

        /// CPU usage percentage for transcription (1-100)
        #[arg(long)]
        cpu: Option<u32>,
    },

    /// Show an episode's summary or transcript
    Show {
        /// Episode ID
        episode_id: i64,

        /// Show transcript instead of summary
        #[arg(short, long)]
        transcript: bool,
    },

    /// Show or update configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Show config file path
    Path,
    /// Set a configuration value
    Set {
        /// Config key (e.g. cpu_percent, whisper_model, language, initial_prompt)
        key: String,
        /// Value to set
        value: String,
    },
}

mod audio;
mod cli;
mod commands;
mod config;
mod db;
mod download;
mod error;
mod feed;
mod models;
mod summarize;
mod transcribe;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command, ConfigAction};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("warn")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let config = config::AppConfig::load()?;

    match &cli.command {
        Command::Add { url } => {
            commands::add::run(url, &config).await?;
        }
        Command::Remove { name, yes, purge } => {
            commands::remove::run(name, *yes, *purge, &config)?;
        }
        Command::List { name } => {
            commands::list::run(name.as_deref(), &config)?;
        }
        Command::Sync {
            name,
            episode,
            download_only,
            redo,
            cpu,
        } => {
            let mut config = config;
            if let Some(pct) = cpu {
                config.transcription.cpu_percent = *pct;
            }
            commands::sync::run(name.as_deref(), *episode, *download_only, *redo, &config).await?;
        }
        Command::Show {
            episode_id,
            transcript,
        } => {
            commands::show::run(*episode_id, *transcript, &config)?;
        }
        Command::Config { action } => {
            match action {
                Some(ConfigAction::Path) => {
                    println!("{}", config::AppConfig::config_path()?.display());
                }
                Some(ConfigAction::Set { key, value }) => {
                    commands::config_set::run(key, value)?;
                }
                Some(ConfigAction::Show) | None => {
                    let content = toml::to_string_pretty(&config)?;
                    println!("{content}");
                }
            }
        }
    }

    Ok(())
}

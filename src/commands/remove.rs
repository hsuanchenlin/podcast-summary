use anyhow::Result;

use crate::config::AppConfig;
use crate::db::Database;

pub fn run(name: &str, yes: bool, _purge: bool, config: &AppConfig) -> Result<()> {
    let db = Database::open(&config.db_path()?)?;

    let podcast = db
        .find_podcast_by_name(name)?
        .ok_or_else(|| anyhow::anyhow!("No podcast matching \"{name}\" found"))?;

    if !yes {
        println!("Remove \"{}\"? [y/N] ", podcast.title);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    db.delete_podcast(podcast.id)?;
    println!("Removed \"{}\"", podcast.title);

    Ok(())
}

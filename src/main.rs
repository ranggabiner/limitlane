use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use directories::ProjectDirs;

use limitlane::cli::args::Cli;
use limitlane::cli::commands::run_cli_command;
use limitlane::storage::db::DatabaseRepository;
use limitlane::storage::keyring::KeyringStore;
use limitlane::tui::run_tui;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let db_path = get_database_path()?;
    let db = DatabaseRepository::open(&db_path)?;
    db.migrate()?;
    let keyring = KeyringStore;

    if cli.command.is_some() {
        run_cli_command(cli, &db, &keyring).await?;
    } else {
        run_tui(Arc::new(db)).await?;
    }

    Ok(())
}

fn get_database_path() -> Result<PathBuf> {
    if let Ok(path_str) = std::env::var("LIMITLANE_DB_PATH") {
        return Ok(PathBuf::from(path_str));
    }

    if let Some(proj_dirs) = ProjectDirs::from("com", "limitlane", "limitlane") {
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;
        Ok(data_dir.join("limitlane.db"))
    } else {
        let fallback = PathBuf::from("limitlane.db");
        Ok(fallback)
    }
}

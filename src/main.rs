use dotenv::dotenv;
use sqlx::sqlite::SqlitePool;

use clap::Parser;

use anyhow::Result;
use tracing::info;

use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

mod discord;
mod poll;
mod www;

#[derive(Parser, Clone)]
struct Config {
    /// Poll current server information
    #[arg(long)]
    poll_servers: bool,

    /// Poll and update cached mod information
    #[arg(long)]
    poll_mods: bool,

    /// Update Discord integration
    #[arg(long)]
    update_discord: bool,

    /// Run web server
    #[arg(long)]
    www: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(true)
        .init();

    let config = Config::parse();

    let pool = SqlitePool::connect(&env::var("DATABASE_URL")?).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let time: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .try_into()
        .unwrap();

    info!("polling start {}", time);

    if config.poll_servers {
        self::poll::update_server_list(&pool, time).await?;
    }
    if config.poll_mods {
        self::poll::update_mods(&pool).await?;
    }
    if config.update_discord {
        self::discord::update_discord(&pool).await?;
    }

    if config.www {
        self::www::run_web_server().await?;
    }

    Ok(())
}

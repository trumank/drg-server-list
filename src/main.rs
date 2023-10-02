#![feature(iter_intersperse)]
use sqlx::sqlite::SqlitePool;
use dotenv::dotenv;

use clap::Parser;

use anyhow::Result;

use std::time::{SystemTime, UNIX_EPOCH};
use std::env;

mod www;
mod poll;
mod discord;

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

    let config = Config::parse();

    let pool = SqlitePool::connect(&env::var("DATABASE_URL")?).await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    let time: i64 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().try_into().unwrap();

    println!("{}", time);

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


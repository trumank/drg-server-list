#![feature(iter_intersperse)]
use sqlx::sqlite::SqlitePool;
use dotenv::dotenv;

use clap::arg;

use anyhow::Result;

use std::time::{SystemTime, UNIX_EPOCH};
use std::env;

mod www;
mod poll;
mod discord;

#[derive(Clone, Copy)]
struct Config {
    poll_servers: bool,
    poll_mods: bool,
    update_discord: bool,
    www: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let matches = clap::Command::new("DRGServerList")
        .about("Standalone DRG server list")
        .arg(arg!(-s --"poll-servers" "Poll current server information"))
        .arg(arg!(-m --"poll-mods"    "Poll and update cached mod information"))
        .arg(arg!(-d --"update-discord" "Update Discord integration"))
        .arg(arg!(-w --"www"          "Run web server"))
        .group(clap::ArgGroup::new("polling")
               .args(&["poll-mods", "poll-servers", "update-discord"])
               .conflicts_with("www")
               .multiple(true))
        .group(clap::ArgGroup::new("action")
               .args(&["poll-mods", "poll-servers", "update-discord", "www"])
               .multiple(true)
               .required(true))
        .get_matches();

    let config = Config {
        poll_servers: matches.is_present("poll-servers"),
        poll_mods: matches.is_present("poll-mods"),
        update_discord: matches.is_present("update-discord"),
        www: matches.is_present("www"),
    };

    let pool = SqlitePool::connect(&env::var("DATABASE_URL")?).await?;

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


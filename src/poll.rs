use anyhow::Result;

use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sqlx::sqlite::SqlitePool;

#[derive(Debug, Deserialize)]
struct ModIoBatchResponse<'a> {
    #[serde(borrow)]
    data: Vec<&'a RawValue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModIoMods {
    data: Vec<ModIoMod>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModIoMod {
    id: i64,
    name: String,
    profile_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Server {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "HostUserID")]
    host_user_id: String,
    #[serde(rename = "DRG_SERVERNAME")]
    server_name: String,
    #[serde(rename = "DRG_SERVERNAME_SAN")]
    server_name_san: String,
    #[serde(rename = "DRG_GLOBALMISSION_SEED")]
    global_mission_seed: i64,
    #[serde(rename = "DRG_MISSION_SEED")]
    mission_seed: i64,
    #[serde(rename = "DRG_DIFF")]
    difficulty: i32,
    #[serde(rename = "DRG_GAMESTATE")]
    gamestate: i32,
    #[serde(rename = "DRG_NUMPLAYERS")]
    player_count: i32,
    #[serde(rename = "DRG_FULL")]
    is_full: i32,
    #[serde(rename = "DRG_REGION")]
    region: String,
    #[serde(rename = "DRG_START")]
    start_time: String,
    #[serde(rename = "DRG_CLASSES")]
    classes: String,
    #[serde(rename = "DRG_CLASSLOCK")]
    class_lock: i32,
    #[serde(rename = "DRG_MISSIONSTRUCTURE")]
    mission_structure: String,
    #[serde(rename = "DRG_PWREQUIRED")]
    password_requires: i32,
    #[serde(rename = "P2PADDR")]
    p2p_address: String,
    #[serde(rename = "P2PPORT")]
    p2p_port: i32,
    #[serde(rename = "Distance")]
    distance: f64,
    #[serde(rename = "Mods")]
    mods: Option<Vec<ServerMod>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerList {
    #[serde(rename = "Lobbies")]
    lobbies: Vec<Server>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ServerMod {
    name: String,
    version: String,
    category: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerListSettings {
    #[serde(rename = "steamTicket")]
    steam_ticket: String,
    #[serde(rename = "steamPingLoc")]
    steam_ping_loc: String,
    #[serde(rename = "gameTypes")]
    game_types: Vec<i32>,
    #[serde(rename = "authenticationTicket")]
    authentication_ticket: String,
    #[serde(rename = "ignoreId")]
    ignore_id: String,
    distance: i32,
    #[serde(rename = "dRG_PWREQUIRED")]
    password_required: i32,
    #[serde(rename = "dRG_REGION")]
    region: String,
    #[serde(rename = "dRG_VERSION")]
    version: Option<i32>,
    #[serde(rename = "difficultyBitset")]
    difficulty_bitset: i32,
    #[serde(rename = "missionSeed")]
    mission_seed: i64,
    #[serde(rename = "globalMissionSeed")]
    global_mission_seed: i64,
    #[serde(rename = "searchString")]
    search_string: String,
    #[serde(rename = "deepDive")]
    deep_dive: bool,
    platform: String,
}

pub async fn update_server_list(pool: &SqlitePool, time: i64) -> Result<()> {
    let mut servers = std::collections::HashMap::<String, Server>::new();

    for server in get_server_list(0b00001).await?.lobbies {
        servers.insert(server.id.to_owned(), server);
    }
    for server in get_server_list(0b00010).await?.lobbies {
        servers.insert(server.id.to_owned(), server);
    }
    for server in get_server_list(0b00100).await?.lobbies {
        servers.insert(server.id.to_owned(), server);
    }
    for server in get_server_list(0b01000).await?.lobbies {
        servers.insert(server.id.to_owned(), server);
    }
    for server in get_server_list(0b10000).await?.lobbies {
        servers.insert(server.id.to_owned(), server);
    }

    for (_, server) in &servers {
        insert_server(pool, time, server).await?;
    }
    Ok(())
}

pub async fn update_mods(pool: &SqlitePool) -> Result<()> {
    sqlx::query!(
        "INSERT OR IGNORE INTO mod (mod_id) SELECT mod_id FROM server_mod WHERE mod_id IS NOT NULL"
    )
    .execute(pool)
    .await?;

    let mod_ids = sqlx::query!("SELECT mod_id FROM mod WHERE metadata IS NULL")
        .fetch_all(pool)
        .await?;
    let id_query: String = mod_ids
        .into_iter()
        .map(|res| res.mod_id.to_string())
        .intersperse(",".into())
        .collect();

    let url = format!(
        "https://api.mod.io/v1/games/2475/mods?api_key={}&id-in={}",
        &std::env::var("MODIO_KEY")?,
        id_query
    );

    let body = reqwest::get(url).await?.text().await?;

    let result: ModIoBatchResponse = serde_json::from_str(&body)?;

    for raw in result.data {
        let metadata = serde_json::to_string(raw)?;
        let m: ModIoMod = serde_json::from_str(raw.get())?;
        sqlx::query!(
            "UPDATE mod SET name = ?, url = ?, metadata = ? WHERE mod_id = ?",
            m.name,
            m.profile_url,
            metadata,
            m.id,
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn insert_server(pool: &SqlitePool, time: i64, server: &Server) -> Result<()> {
    sqlx::query!(
        r#"
INSERT INTO server (
    time,
    lobby_id,
    host_user_id,
    server_name,
    server_name_san,
    global_mission_seed,
    mission_seed,
    diff,
    gamestate,
    numplayers,
    full,
    region,
    start,
    classes,
    classlock,
    mission_structure,
    password,
    p2paddress,
    p2pport,
    distance
)
VALUES ( ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ? )
        "#,
        time,
        server.id,
        server.host_user_id,
        server.server_name,
        server.server_name_san,
        server.global_mission_seed,
        server.mission_seed,
        server.difficulty,
        server.gamestate,
        server.player_count,
        server.is_full,
        server.region,
        server.start_time,
        server.classes,
        server.class_lock,
        server.mission_structure,
        server.password_requires,
        server.p2p_address,
        server.p2p_port,
        server.distance
    )
    .execute(pool)
    .await?;

    if let Some(mods) = &server.mods {
        for m in mods {
            insert_server_mod(pool, time, server, m).await?;
        }
    }

    Ok(())
}

async fn insert_server_mod(
    pool: &SqlitePool,
    time: i64,
    server: &Server,
    m: &ServerMod,
) -> Result<()> {
    if let Err(..) = &m.name.parse::<i64>() {
        println!("Mod has non-numeric ID: {}", m.name);
        return Ok(());
    }

    sqlx::query!(
        r#"
INSERT INTO server_mod (
    time,
    lobby_id,
    mod_id,
    version,
    category
)
VALUES ( ?, ?, ?, ?, ? )
        "#,
        time,
        server.id,
        m.name,
        m.version,
        m.category
    )
    .execute(pool)
    .await?;
    sqlx::query!("INSERT OR IGNORE INTO mod (mod_id) VALUES ( ? )", m.name)
        .execute(pool)
        .await?;
    Ok(())
}

async fn get_server_list(difficulty_bitset: u8) -> Result<ServerList> {
    let settings = ServerListSettings {
        steam_ticket: "".into(),
        steam_ping_loc: "".into(),
        game_types: [1, 2, 0, 99].to_vec(),
        authentication_ticket: "OtherPlatform".into(),
        ignore_id: "".into(),
        distance: 3,
        password_required: 0,
        region: "".into(),
        version: None,
        difficulty_bitset: difficulty_bitset as i32,
        mission_seed: 0,
        global_mission_seed: 0,
        search_string: "".into(),
        deep_dive: false,
        platform: "steam".into(),
    };

    let result: ServerList = reqwest::Client::new()
        .post("https://drg.ghostship.dk/steam/games/list2")
        .json(&settings)
        .send()
        .await?
        .json()
        .await?;
    Ok(result)
}

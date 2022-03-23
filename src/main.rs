#![feature(iter_intersperse)]
use sqlx::sqlite::SqlitePool;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use reqwest;
use dotenv::dotenv;

use trillium::{Conn, State, Handler};
use trillium_logger::Logger;
use trillium_router::Router;
use trillium_static_compiled::static_compiled;
use maud::{DOCTYPE, html, PreEscaped};

use anyhow::Result;

use std::time::{SystemTime, UNIX_EPOCH};
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let pool = SqlitePool::connect(&env::var("DATABASE_URL")?).await?;

    let time: i64 = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().try_into().unwrap();

    println!("{}", time);

    //update_server_list(&pool, time).await?;
    //update_mods(&pool).await?;

    run_web_server().await?;

    Ok(())
}

async fn run_web_server() -> Result<()>{
    //trillium_tokio::run_async(|conn: trillium::Conn| async move {
        //conn.ok("hello from trillium!")
    //}).await;

    let pool = SqlitePool::connect(&env::var("DATABASE_URL")?).await?;
    let db = DB::new(pool);

    trillium_tokio::config()
        .run_async(app(db)).await;

    Ok(())
}

#[derive(Debug, Clone)]
struct DB(std::sync::Arc<tokio::sync::Mutex<SqlitePool>>);
impl DB {
    fn new(pool: SqlitePool) -> DB {
        DB(std::sync::Arc::new(tokio::sync::Mutex::new(pool)))
    }
}

fn app<'a>(db: DB) -> impl Handler {
    (
        Logger::new(),
        State::new(db),
        router(),
        static_compiled!("./public"),
    )
}

trait MaudConnExt {
    fn render(self, template: PreEscaped<String>) -> Self;
}

impl MaudConnExt for Conn {
    fn render(self, template: PreEscaped<String>) -> Self {
        self.ok(template.0)
    }
}

fn router() -> impl Handler {
    Router::new()
        .get("/", |mut conn: Conn| async move {
            let html = {
                let mut db = conn.state_mut::<DB>().unwrap().0.lock().await.acquire().await.unwrap();
                let res = sqlx::query!(
                    r#"SELECT datetime(time, 'unixepoch', 'localtime') AS time,
                        lobby_id,
                        diff,
                        region,
                        host_user_id,
                        server_name,
                        (SELECT json_group_array(json_object('id', mod_id, 'category', category, 'name', name, 'url', url)) FROM
                            (SELECT mod_id, category, name, url
                            FROM server_mod
                            JOIN mod USING(mod_id)
                            WHERE
                                server_mod.time = server.time
                                AND server_mod.lobby_id = server.lobby_id
                                AND category != 0
                            ORDER BY category)
                        ) AS mods
                        FROM server
                        WHERE diff = 4
                        ORDER BY time
                        DESC LIMIT 1000;
                    "#
                )
                .fetch_all(&mut db)
                .await.unwrap();

                #[derive(Serialize, Deserialize)]
                struct Mod {
                    id: i64,
                    category: Option<i32>,
                    name: Option<String>,
                    url: Option<String>,
                }

                html! {
                    html lang="en" {
                        (DOCTYPE)
                        head {
                            meta charset="utf-8";
                            meta name="viewport" content="width=device-width, initial-scale=1";
                            link href="/static/css/bootstrap.min.css" rel="stylesheet";
                            style {
                                (PreEscaped(r#"
                                    body > ul {
                                        max-width: 700px;
                                        width: auto;
                                        margin: 0 auto;
                                    }
                                "#))
                            }
                        }
                        body {
                            ul.list-group {
                                @for l in res {
                                    li.list-group-item {
                                        div.d-flex."gap-2"."w-100".justify-content-between {
                                            div {
                                                h6 {
                                                    (format!("Hazard {}", l.diff + 1))
                                                    " - "
                                                    a href=(format!("steam://joinlobby/548430/{}/{}", l.lobby_id, l.host_user_id)) {
                                                        (l.server_name)
                                                    }
                                                }
                                                p."mb-0"."opacity-75" {
                                                    a href=(format!("https://steamcommunity.com/profiles/{}", l.host_user_id)) {
                                                        "Steam profile"
                                                    }
                                                    @if let Some(json) = l.mods {
                                                        @if let Ok(mods) = serde_json::from_str::<Vec<Mod>>(&json) {
                                                            ul {
                                                                @for m in mods {
                                                                    li {
                                                                        @if let (Some(category), Some(url), Some(name)) = (m.category, m.url, m.name) {
                                                                            (match category {
                                                                                0 => "Verified",
                                                                                1 => "Approved",
                                                                                2 => "Sandbox",
                                                                                _ => "Unknown"
                                                                            })
                                                                            " - "
                                                                            a href=(url) {
                                                                                (name)
                                                                            }
                                                                        } @else {
                                                                            "Hidden mod ("(m.id)")"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            small."opacity-50".text-nowrap {
                                                (l.time.unwrap())
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        //(res.iter().map(|r| format!("{:?}", r)).join("\n"))
                    }
                }
            };
            conn.render(html)
        })
}

async fn update_server_list(pool: &SqlitePool, time: i64) -> Result<()> {
    let mut servers = std::collections::HashMap::<String, Server>::new();

    for server in get_server_list(0b00001).await?.Lobbies { servers.insert(server.Id.to_owned(), server); }
    for server in get_server_list(0b00010).await?.Lobbies { servers.insert(server.Id.to_owned(), server); }
    for server in get_server_list(0b00100).await?.Lobbies { servers.insert(server.Id.to_owned(), server); }
    for server in get_server_list(0b01000).await?.Lobbies { servers.insert(server.Id.to_owned(), server); }
    for server in get_server_list(0b10000).await?.Lobbies { servers.insert(server.Id.to_owned(), server); }

    for (_, server) in &servers {
        insert_server(pool, time, server).await?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ModIoBatchResponse<'a> {
    #[serde(borrow)]
    data: Vec<&'a RawValue>,
}

async fn update_mods(pool: &SqlitePool) -> Result<()> {
    sqlx::query!("INSERT OR IGNORE INTO mod (mod_id) SELECT mod_id FROM server_mod WHERE mod_id IS NOT NULL").execute(pool).await?;

    let mod_ids = sqlx::query!("SELECT mod_id FROM mod WHERE metadata IS NULL").fetch_all(pool).await?;
    let id_query: String = mod_ids.into_iter().map(|res| res.mod_id.to_string()).intersperse(",".into()).collect();

    let url = format!("https://api.mod.io/v1/games/2475/mods?api_key={}&id-in={}", &env::var("MODIO_KEY")?, id_query);

    let body = reqwest::get(url)
        .await?
        .text()
        .await?;

    let result: ModIoBatchResponse = serde_json::from_str(&body)?;

    for raw in result.data {
        let metadata = serde_json::to_string(raw)?;
        let m: ModIoMod = serde_json::from_str(raw.get())?;
        sqlx::query!("UPDATE mod SET name = ?, url = ?, metadata = ? WHERE mod_id = ?",
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
    Id: String,
    HostUserID: String,
    DRG_SERVERNAME: String,
    DRG_SERVERNAME_SAN: String,
    DRG_GLOBALMISSION_SEED: i64,
    DRG_MISSION_SEED: i64,
    DRG_DIFF: i32,
    DRG_GAMESTATE: i32,
    DRG_NUMPLAYERS: i32,
    DRG_FULL: i32,
    DRG_REGION: String,
    DRG_START: String,
    DRG_CLASSES: String,
    DRG_CLASSLOCK: i32,
    DRG_MISSIONSTRUCTURE: String,
    DRG_PWREQUIRED: i32,
    P2PADDR: String,
    P2PPORT: i32,
    Distance: f64,
    Mods: Option<Vec<ServerMod>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerList {
    Lobbies: Vec<Server>
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerMod {
    Name: String,
    Version: String,
    Category: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerListSettings {
    steamTicket: String,
    steamPingLoc: String,
    gameTypes: Vec<i32>,
    authenticationTicket: String,
    ignoreId: String,
    distance: i32,
    dRG_PWREQUIRED: i32,
    dRG_REGION: String,
    dRG_VERSION: i32,
    difficultyBitset: i32,
    missionSeed: i64,
    globalMissionSeed: i64,
    searchString: String,
    deepDive: bool,
    platform: String,
}

async fn insert_server_list(pool: &SqlitePool, time: i64, servers: &ServerList) -> Result<()> {
    for server in &servers.Lobbies {
        insert_server(pool, time, server).await?;
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
        server.Id,
        server.HostUserID,
        server.DRG_SERVERNAME,
        server.DRG_SERVERNAME_SAN,
        server.DRG_GLOBALMISSION_SEED,
        server.DRG_MISSION_SEED,
        server.DRG_DIFF,
        server.DRG_GAMESTATE,
        server.DRG_NUMPLAYERS,
        server.DRG_FULL,
        server.DRG_REGION,
        server.DRG_START,
        server.DRG_CLASSES,
        server.DRG_CLASSLOCK,
        server.DRG_MISSIONSTRUCTURE,
        server.DRG_PWREQUIRED,
        server.P2PADDR,
        server.P2PPORT,
        server.Distance
    )
    .execute(pool)
    .await?;

    if let Some(mods) = &server.Mods {
        for m in mods {
            insert_server_mod(pool, time, server, m).await?;
        }
    }

    Ok(())
}

async fn insert_server_mod(pool: &SqlitePool, time: i64, server: &Server, m: &ServerMod) -> Result<()> {

    if let Err(..) = &m.Name.parse::<i64>() {
        println!("Mod has non-numeric ID: {}", m.Name);
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
        server.Id,
        m.Name,
        m.Version,
        m.Category
    )
    .execute(pool)
    .await?;
    sqlx::query!("INSERT OR IGNORE INTO mod (mod_id) VALUES ( ? )", m.Name).execute(pool).await?;
    Ok(())
}

async fn get_server_list(difficulty_bitset: u8) -> Result<ServerList> {
    let settings = ServerListSettings {
        steamTicket: "".into(),
        steamPingLoc: "".into(),
        gameTypes: [1,2,0,99].to_vec(),
        authenticationTicket: "OtherPlatform".into(),
        ignoreId: "".into(),
        distance: 3,
        dRG_PWREQUIRED: 0,
        dRG_REGION: "".into(),
        dRG_VERSION: 67712,
        difficultyBitset: difficulty_bitset as i32,
        missionSeed: 0,
        globalMissionSeed: 0,
        searchString: "".into(),
        deepDive: false,
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

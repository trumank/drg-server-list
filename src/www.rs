use sqlx::sqlite::SqlitePool;
use serde::{Deserialize, Serialize};

use std::env;

use anyhow::Result;

use trillium::{conn_unwrap, Conn, State, Handler};
use trillium_logger::Logger;
use trillium_router::{Router, RouterConnExt};
use trillium_static_compiled::static_compiled;
use maud::{DOCTYPE, html, PreEscaped};

pub async fn run_web_server() -> Result<()>{
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

struct Server {
    time: i64,
    time_formatted: String,
    lobby_id: String,
    difficulty: i64,
    region: String,
    host_user_id: String,
    server_name: String,
    mods: Vec<Mod>,
}

#[derive(Serialize, Deserialize)]
struct Mod {
    id: i64,
    category: Option<i32>,
    name: Option<String>,
    url: Option<String>,
}

fn router() -> impl Handler {
    Router::new()
        .get("/", |mut conn: Conn| async move {
            let mut db = conn.state_mut::<DB>().unwrap().0.lock().await.acquire().await.unwrap();
            let res = sqlx::query!(
                r#"SELECT time,
                    datetime(time, 'unixepoch', 'localtime') AS "time_formatted!: String",
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

            /*
            let servers: Vec<Server> = res.into_iter().map(|r| Ok(Server {
                time: r.time.ok_or(Err("Malformed time"))?,
                lobby_id: r.lobby_id,
                difficulty: r.diff,
                region: r.region,
                host_user_id: r.host_user_id,
                server_name: r.server_name,
                mods: r.mods.map_or_else(|| Ok(vec![]), |m| {
                    serde_json::from_str::<Vec<Mod>>(&m)
                })?
            })).collect()?;
            */

            let servers: Vec<Server> = res.into_iter().map(|r| Server {
                time: r.time,
                time_formatted: r.time_formatted,
                lobby_id: r.lobby_id,
                difficulty: r.diff,
                region: r.region,
                host_user_id: r.host_user_id,
                server_name: r.server_name,
                mods: r.mods.map_or_else(|| Ok(vec![]), |m| {
                    serde_json::from_str::<Vec<Mod>>(&m)
                }).unwrap()
            }).collect();

            conn.render(render_servers(servers))
        })
        .get("/server/:time/:lobby_id", |mut conn: Conn| async move {
            let time = conn_unwrap!(conn.param("time"), conn).to_owned();
            let lobby_id = conn_unwrap!(conn.param("lobby_id"), conn).to_owned();

            let mut db = conn.state_mut::<DB>().unwrap().0.lock().await.acquire().await.unwrap();
            let res = sqlx::query!(
                r#"SELECT time,
                    datetime(time, 'unixepoch', 'localtime') AS "time_formatted!: String",
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
                    ) AS "mods?: String"
                    FROM server
                    WHERE server.time = ? AND server.lobby_id = ?
                    ORDER BY time
                "#,
                time,
                lobby_id,
            )
            .fetch_all(&mut db)
            .await.unwrap();

            /*
            let servers: Vec<Server> = res.into_iter().map(|r| Ok(Server {
                time: r.time.ok_or(Err("Malformed time"))?,
                lobby_id: r.lobby_id,
                difficulty: r.diff,
                region: r.region,
                host_user_id: r.host_user_id,
                server_name: r.server_name,
                mods: r.mods.map_or_else(|| Ok(vec![]), |m| {
                    serde_json::from_str::<Vec<Mod>>(&m)
                })?
            })).collect()?;
            */

            let servers: Vec<Server> = res.into_iter().map(|r| Server {
                time: r.time,
                time_formatted: r.time_formatted,
                lobby_id: r.lobby_id,
                difficulty: r.diff,
                region: r.region,
                host_user_id: r.host_user_id,
                server_name: r.server_name,
                mods: r.mods.map_or_else(|| Ok(vec![]), |m| {
                    serde_json::from_str::<Vec<Mod>>(&m)
                }).unwrap()
            }).collect();

            conn.render(render_servers(servers))
        })
}

fn render_servers(servers: Vec<Server>) -> PreEscaped<String> {
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
                    @for server in servers {
                        (render_server(server))
                    }
                }
            }
        }
    }
}

fn render_server(server: Server) -> PreEscaped<String> {
    html! {
        li.list-group-item {
            div.d-flex."gap-2"."w-100".justify-content-between {
                div {
                    h6 {
                        (format!("Hazard {}", server.difficulty + 1))
                        " - "
                        a href=(format!("steam://joinlobby/548430/{}/{}", server.lobby_id, server.host_user_id)) {
                            (server.server_name)
                        }
                    }
                    p."mb-0"."opacity-75" {
                        a href=(format!("https://steamcommunity.com/profiles/{}", server.host_user_id)) {
                            "Steam profile"
                        }
                        " - "
                        a href=(format!("/server/{}/{}", server.time, server.lobby_id)) {
                            "Link"
                        }
                        ul {
                            @for m in server.mods {
                                @if let Some(category) = m.category {
                                    li {
                                        (match category {
                                            0 => "Verified",
                                            1 => "Approved",
                                            2 => "Sandbox",
                                            _ => "Unknown"
                                        })
                                        " - "
                                        @if let (Some(url), Some(name)) = (m.url, m.name) {
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
                small."opacity-50".text-nowrap {
                    (server.time_formatted)
                }
            }
        }
    }
}

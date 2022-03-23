use sqlx::sqlite::SqlitePool;
use serde::{Deserialize, Serialize};

use std::env;

use anyhow::Result;

use trillium::{Conn, State, Handler};
use trillium_logger::Logger;
use trillium_router::Router;
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

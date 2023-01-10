use sqlx::sqlite::SqlitePool;
use serde::{Deserialize, Serialize};

use lazy_static::lazy_static;
use regex::Regex;

use anyhow::{anyhow, Result};

#[derive(Debug, Serialize, Deserialize)]
struct Mod {
    id: i64,
    category: Option<i32>,
    name: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum WebhookResponse {
    Success {
        id: String,
    },
    Ratelimit {
        global: bool,
        message: String,
        retry_after: f32,
    },
    Error {
        message: String,
        code: i32,
    },
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookBody {
    pub avatar_url: Option<String>,
    pub embeds: Vec<WebhookEmbed>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookEmbed {
    pub title: String,
    pub author: WebhookAuthor,
    pub description: String,
    pub fields: Vec<WebhookField>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookAuthor {
    pub name: String,
    pub icon_url: String,
    pub url: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebhookField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteamPlayerRequest {
    pub response: SteamPlayerResponse,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteamPlayerResponse {
    pub players: Vec<SteamPlayer>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteamPlayer {
    pub steamid: String,
    pub communityvisibilitystate: i64,
    pub profilestate: Option<i64>,
    pub personaname: String,
    pub profileurl: String,
    pub avatar: String,
    pub avatarmedium: String,
    pub avatarfull: String,
    pub avatarhash: String,
    pub personastate: i64,
    pub primaryclanid: Option<String>,
    pub timecreated: Option<i64>,
    pub personastateflags: Option<i64>,
}

fn format_mod_field(mods: &Vec<Mod>, category: i32, name: &str) -> Option<WebhookField> {
    let mut value = String::with_capacity(1000);
    let filtered_mods: Vec<&Mod> = mods.iter().filter(|m| m.category == Some(category)).collect();
    // TODO: Very messy and probably broken
    for (i, m) in filtered_mods.iter().enumerate() {
        let formatted = if let (Some(url), Some(name)) = (m.url.as_ref(), m.name.as_ref()) {
            format!("[{}]({})", name, url)
        } else {
            "Hidden mod".to_string()
        };
        if formatted.chars().count() + value.chars().count() > value.capacity() {
            value.push_str(&format!("...and {} more", filtered_mods.len() - i));
            break;
        }
        value.push_str(&formatted);
        value.push_str("\n");
    }
    if value != "" {
        Some(WebhookField {
            name: name.to_string(),
            value,
            inline: true,
        })
    } else {
        None
    }
}

fn format_classes(classes: &String) -> String {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"\d+;").unwrap();
    }
    let mut vec: Vec<&str> = RE.find_iter(&classes)
        .map(|c| match c.as_str() {
            "0;" => "<:driller:964680901621612584>",
            "1;" => "<:engineer:964680922920255548>",
            "2;" => "<:gunner:964680948530704404>",
            "3;" => "<:scout:964680965521813524>",
            _ => "<unknown>",
        })
        .collect();
    while vec.len() < 4 {
        vec.push("<:empty:964681045347823616>");
    }
    vec.join("")
}

pub async fn parse_response<T: serde::de::DeserializeOwned>(res: reqwest::Response) -> Result<T> {
    let text = res.text().await?;
    match serde_json::from_str::<T>(&text) {
        Ok(json) => Ok(json),
        Err(e) => Err(anyhow!("{}\nRaw string: {}", e, text)),
    }
}

pub async fn ratelimit_sleep(res: &reqwest::Response) {
    let headers = res.headers();
    if let (Some(remaining), Some(reset)) = (
        headers.get("x-ratelimit-remaining").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<i32>().ok()),
        headers.get("x-ratelimit-reset-after").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<f32>().ok())) {
        if remaining == 0 {
            println!("Sleeping for: {}", reset);
            tokio::time::sleep(tokio::time::Duration::from_secs_f32(reset)).await;
        } else {
            println!("Requests remaining: {}", remaining);
        }
    }
}

pub async fn update_discord(pool: &SqlitePool) -> Result<()> {
    let webhook = &std::env::var("DISCORD_WEBHOOK").unwrap();
    let steam_key = &std::env::var("STEAM_WEB_KEY").unwrap();

    let res = sqlx::query!(
        r#"SELECT time,
            datetime(time, 'unixepoch', 'localtime') AS "time_formatted!: String",
            lobby_id,
            diff,
            region,
            host_user_id,
            server_name,
            classes,
            start,
            (SELECT json_group_array(json_object('id', mod_id, 'category', category, 'name', name, 'url', url)) FROM
                (SELECT mod_id, category, name, url
                FROM server_mod
                JOIN mod USING(mod_id)
                WHERE
                    server_mod.time = server.time
                    AND server_mod.lobby_id = server.lobby_id
                ORDER BY category)
            ) AS "mods?: String",
            (SELECT message_id FROM discord_message WHERE server.lobby_id = discord_message.lobby_id) AS "message_id?" -- use subquery because sqlx can't handle left join
            FROM server
            WHERE (server.time, server.lobby_id) IN (
                SELECT time, lobby_id
                FROM server
                JOIN server_mod USING(time, lobby_id)
                WHERE
                    mod_id IN (
                        1861561,
                        1897251,
                        1775635,
                        1137703,
                        1137738,
                        1143817,
                        1729804,
                        1703369,
                        1137776,
                        1727230,
                        1981468, -- More Mutators
                        1962912, -- Buyable Missions
                        2093114 -- Mission Randomizer
                    )
                    AND (server.time, server.lobby_id) NOT IN (
                        SELECT MAX(time), lobby_id
                        FROM server_mod
                        WHERE mod_id IN (
                            1034411, -- 2x flashlight
                            1034683, -- 3x flashlight
                            1034060, -- 5x flashlight
                            1176984, -- better minigun
                            1159061 -- better scout
                        )
                        GROUP BY lobby_id
                    )
                    AND (server.time, server.lobby_id) IN (
                        SELECT MAX(time), lobby_id
                        FROM server
                        WHERE time > strftime('%s', datetime('now', '-10 minutes'))
                        GROUP BY lobby_id
                    )
            )
            ORDER BY time
        "#,
    )
    .fetch_all(pool)
    .await.unwrap();

    for server in res {
        let mods = server.mods.map_or_else(|| Ok(vec![]), |m| {
            serde_json::from_str::<Vec<Mod>>(&m)
        }).unwrap();

        let mut fields = vec![
            WebhookField {
                name: "Region".to_string(),
                value: server.region,
                inline: true,
            },
            WebhookField {
                name: "Difficulty".to_string(),
                value: format!("Hazard {}", server.diff + 1),
                inline: true,
            },
            WebhookField {
                name: "Classes".to_string(),
                value: format_classes(&server.classes),
                inline: true,
            },
            WebhookField {
                name: "Status".to_string(),
                value: (if server.start.is_empty() { "In Space Rig".to_string() } else { "In Mission".to_string() }),
                inline: false,
            },
            /*WebhookField {
                name: "\u{200B}".to_string(),
                value: "\u{200B}".to_string(),
                inline: false,
            },*/
        ];

        if let Some(field) = format_mod_field(&mods, 0, "Verified Mods") { fields.push(field) }
        if let Some(field) = format_mod_field(&mods, 1, "Approved Mods") { fields.push(field) }
        if let Some(field) = format_mod_field(&mods, 2, "Sandboxed Mods") { fields.push(field) }

        let result: SteamPlayerRequest = parse_response(reqwest::Client::new()
                .get(format!("https://api.steampowered.com/ISteamUser/GetPlayerSummaries/v0002/?key={}&steamids={}", steam_key, server.host_user_id))
                .send()
                .await?
            ).await?;

        let player = &result.response.players[0];

        let data = WebhookBody {
            avatar_url: Some("https://cdn.discordapp.com/attachments/878318716801155236/968174640847523930/engo.png".to_string()),
            embeds: vec![
                WebhookEmbed {
                    title: server.server_name,
                    author: WebhookAuthor {
                        name: player.personaname.to_owned(),
                        icon_url: player.avatarfull.to_owned(),
                        url: format!("https://steamcommunity.com/profiles/{}", server.host_user_id),
                    },
                    description: format!("steam://joinlobby/548430/{}/{}", server.lobby_id, server.host_user_id),
                    fields: fields,
                }
            ]
        };

        let mut success = false;

        while !success {
            let client = reqwest::Client::new();
            let res = if let Some(message) = &server.message_id {
                    client.patch(format!("{}/messages/{}?wait=true", webhook, message))
                } else {
                    client.post(format!("{}?wait=true", webhook))
                }
                .json(&data)
                .send()
                .await?;
            ratelimit_sleep(&res).await;

            let result: WebhookResponse = parse_response(res).await?;

            match result {
                WebhookResponse::Success{id} => {
                    sqlx::query!("INSERT INTO discord_message(message_id, lobby_id, last_updated) VALUES (?, ?, strftime('%s', 'now')) ON CONFLICT(message_id) DO UPDATE SET last_updated = excluded.last_updated;", id, server.lobby_id)
                        .execute(pool)
                        .await?;
                    success = true;
                },
                WebhookResponse::Ratelimit{message, global, retry_after} => {
                    println!("{}; global: {}, retry_after: {}", message, global, retry_after);
                    tokio::time::sleep(tokio::time::Duration::from_secs_f32(retry_after)).await;
                },
                WebhookResponse::Error{message, code} => {
                    if code == 10008 {
                        if let Some(id) = &server.message_id {
                            println!("Tried to update unknown message. Deleting...");
                            sqlx::query!("DELETE FROM discord_message WHERE message_id = ?", id)
                                .execute(pool)
                                .await?;
                        }
                    } else {
                        println!("Received error from endpoint: {} code: {}", message, code);
                    }
                    success = true;
                },
            }
        }
    }

    let res = sqlx::query!(
        r#"SELECT message_id
            FROM discord_message
            WHERE last_updated <= strftime('%s', datetime('now', '-10 minutes'))
        "#,
    )
    .fetch_all(pool)
    .await.unwrap();

    for message in res {
        let res = reqwest::Client::new()
            .delete(format!("{}/messages/{}", webhook, message.message_id))
            .send()
            .await?;
        ratelimit_sleep(&res).await;

        sqlx::query!("DELETE FROM discord_message WHERE message_id = ?", message.message_id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

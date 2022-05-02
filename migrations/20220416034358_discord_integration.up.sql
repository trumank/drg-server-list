CREATE TABLE IF NOT EXISTS discord_message (
    message_id           TEXT NOT NULL PRIMARY KEY,
    lobby_id             TEXT NOT NULL UNIQUE,
    last_updated         INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
) STRICT;

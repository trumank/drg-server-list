CREATE TABLE IF NOT EXISTS server (
    time                 INTEGER NOT NULL,
    lobby_id             TEXT NOT NULL,
    host_user_id         TEXT NOT NULL,
    server_name          TEXT NOT NULL,
    server_name_san      TEXT NOT NULL,
    global_mission_seed  TEXT NOT NULL,
    mission_seed         TEXT NOT NULL,
    diff                 INTEGER NOT NULL,
    gamestate            INTEGER NOT NULL,
    numplayers           INTEGER NOT NULL,
    full                 INTEGER NOT NULL,
    region               TEXT NOT NULL,
    start                TEXT NOT NULL,
    classes              TEXT NOT NULL,
    classlock            INTEGER NOT NULL,
    mission_structure    TEXT NOT NULL,
    password             INTEGER NOT NULL,
    p2paddress           TEXT NOT NULL,
    p2pport              INTEGER NOT NULL,
    distance             REAL NOT NULL,
    PRIMARY KEY (time, lobby_id)
) STRICT;

CREATE TABLE IF NOT EXISTS server_mod (
    time                 INTEGER NOT NULL,
    lobby_id             TEXT NOT NULL,
    mod_id               INTEGER NOT NULL,
    version              TEXT NOT NULL,
    category             INTEGER NOT NULL,
    PRIMARY KEY (time, lobby_id, mod_id),
    FOREIGN KEY (time, lobby_id) REFERENCES server (time, lobby_id)
) STRICT;

CREATE TABLE IF NOT EXISTS mod (
    mod_id INTEGER PRIMARY KEY NOT NULL,
    name TEXT,
    url TEXT
) STRICT;

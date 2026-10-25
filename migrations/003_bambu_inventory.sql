-- Materials are now derived from the spools loaded in the AMS. The spool
-- weight moves to the printers, and two products may share a display
-- label, so the table is rebuilt without UNIQUE(name, color).
-- Existing rows are kept (orders reference them) but start unavailable:
-- the next AMS sync re-enables the ones that are loaded.
PRAGMA foreign_keys = OFF;

CREATE TABLE materials_new
(
    id        INTEGER PRIMARY KEY,
    name      TEXT    NOT NULL,
    color     TEXT    NOT NULL,
    color_hex TEXT,
    bambu_key TEXT    UNIQUE,
    available INTEGER NOT NULL DEFAULT 0
);

INSERT INTO materials_new (id, name, color, available)
SELECT id, name, color, 0 FROM materials;

DROP TABLE materials;
ALTER TABLE materials_new RENAME TO materials;

PRAGMA foreign_keys = ON;

CREATE TABLE bambu_account
(
    id           INTEGER PRIMARY KEY CHECK (id = 1),
    user_id      TEXT NOT NULL,
    access_token TEXT NOT NULL,
    devices_json TEXT NOT NULL,
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

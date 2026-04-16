CREATE TABLE users
(
    id           INTEGER PRIMARY KEY,
    cas_login    TEXT    UNIQUE NOT NULL,
    display_name TEXT    NOT NULL,
    email        TEXT    NOT NULL,
    phone        TEXT,
    promo        TEXT,
    created_at   TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE materials
(
    id        INTEGER PRIMARY KEY,
    name      TEXT    NOT NULL,
    color     TEXT    NOT NULL,
    available INTEGER NOT NULL DEFAULT 1,
    UNIQUE(name, color)
);

CREATE TABLE orders
(
    id                  INTEGER PRIMARY KEY,
    user_id             INTEGER NOT NULL REFERENCES users(id),
    created_at          TEXT    NOT NULL DEFAULT (datetime('now')),
    software_used       TEXT    NOT NULL,
    material_id         INTEGER REFERENCES materials(id),
    quantity            INTEGER NOT NULL DEFAULT 1,
    comments            TEXT,
    status              TEXT    NOT NULL DEFAULT 'a_traiter'
                        CHECK (status IN ('a_traiter', 'en_traitement', 'imprime', 'livre', 'annule')),
    requires_payment    INTEGER NOT NULL DEFAULT 0,
    sliced_weight_grams REAL,
    print_time_minutes  INTEGER
);

CREATE TABLE admin_users
(
    id            INTEGER PRIMARY KEY,
    login         TEXT    UNIQUE NOT NULL,
    password_hash TEXT    NOT NULL
);

CREATE TABLE order_files
(
    id                INTEGER PRIMARY KEY,
    order_id          INTEGER NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    original_filename TEXT    NOT NULL,
    stored_filename   TEXT    NOT NULL UNIQUE,
    size_bytes        INTEGER NOT NULL,
    mime_type         TEXT    NOT NULL,
    uploaded_at       TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_order_files_order_id ON order_files(order_id);
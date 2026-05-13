CREATE TABLE IF NOT EXISTS alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    ticker TEXT NOT NULL,
    quote_currency TEXT,
    condition_kind TEXT NOT NULL,
    threshold_amount TEXT NOT NULL,
    threshold_currency TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    cooldown_secs INTEGER NOT NULL DEFAULT 60,
    last_fired_at TEXT
);

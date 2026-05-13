CREATE TABLE IF NOT EXISTS watchlist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    ticker TEXT NOT NULL,
    quote_currency TEXT,
    position INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (CURRENT_TIMESTAMP),
    UNIQUE(kind, ticker, quote_currency)
);

CREATE TABLE IF NOT EXISTS holdings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    ticker TEXT NOT NULL,
    quote_currency TEXT,
    quantity TEXT NOT NULL,            -- decimal as string
    avg_cost_amount TEXT NOT NULL,
    avg_cost_currency TEXT NOT NULL,
    UNIQUE(kind, ticker, quote_currency)
);

CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    poll_interval_secs INTEGER NOT NULL,
    display_currency TEXT NOT NULL,
    theme TEXT NOT NULL,
    widget_opacity REAL NOT NULL,
    widget_always_on_top INTEGER NOT NULL
);

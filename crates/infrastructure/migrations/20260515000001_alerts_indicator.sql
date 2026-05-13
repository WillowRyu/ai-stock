-- Relax the alerts schema: threshold_amount and threshold_currency become
-- nullable so we can store indicator-based alerts (RSI thresholds, MACD
-- crosses with no threshold) alongside price thresholds. SQLite cannot
-- drop NOT NULL in place, so recreate the table.
CREATE TABLE alerts_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    ticker TEXT NOT NULL,
    quote_currency TEXT,
    condition_kind TEXT NOT NULL,
    threshold_amount TEXT,
    threshold_currency TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    cooldown_secs INTEGER NOT NULL DEFAULT 60,
    last_fired_at TEXT
);

INSERT INTO alerts_new (id, kind, ticker, quote_currency, condition_kind, threshold_amount, threshold_currency, enabled, cooldown_secs, last_fired_at)
SELECT id, kind, ticker, quote_currency, condition_kind, threshold_amount, threshold_currency, enabled, cooldown_secs, last_fired_at FROM alerts;

DROP TABLE alerts;
ALTER TABLE alerts_new RENAME TO alerts;

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ipc;
mod tauri_notifier;
mod wiring;

use std::time::Duration;
use tauri::{Emitter, Manager};

fn apply_window_effects(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        let _ = apply_vibrancy(
            window,
            NSVisualEffectMaterial::Sidebar,
            Some(NSVisualEffectState::Active),
            Some(10.0),
        );
    }
    #[cfg(target_os = "windows")]
    {
        use window_vibrancy::apply_mica;
        let _ = apply_mica(window, Some(true));
    }
    #[cfg(target_os = "linux")]
    {
        let _ = window;
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            ipc::watchlist_get, ipc::watchlist_add, ipc::watchlist_remove,
            ipc::quotes_snapshot,
            ipc::portfolio_upsert, ipc::portfolio_delete, ipc::portfolio_valuation,
            ipc::settings_get, ipc::settings_save,
            ipc::widget_toggle,
            ipc::indicators_for,
            ipc::chart_data,
            ipc::alerts_list, ipc::alerts_create, ipc::alerts_delete,
            ipc::ai_set_key, ipc::ai_clear_key, ipc::ai_has_key,
            ipc::ai_start_turn, ipc::ai_send_message, ipc::ai_cancel,
            ipc::kis_set_credentials, ipc::kis_clear_credentials, ipc::kis_has_credentials,
            ipc::set_window_theme,
        ])
        .setup(|app| {
            if let Some(main_window) = app.get_webview_window("main") {
                apply_window_effects(&main_window);
            }
            if let Some(widget_window) = app.get_webview_window("widget") {
                apply_window_effects(&widget_window);
            }

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let db_path = handle
                    .path()
                    .app_data_dir()
                    .expect("app data dir")
                    .join("ai-stock.db");
                std::fs::create_dir_all(db_path.parent().unwrap()).ok();
                let state = wiring::assemble(handle.clone(), db_path, std::env::var("FINNHUB_API_KEY").ok()).await;
                handle.manage(state);

                // Periodic refresh + emit loop. This loop owns the polling cadence
                // so it can capture per-symbol provider errors from `RefreshOutcome`
                // and emit them to the UI as `provider-error` events alongside the
                // usual `quote-update` snapshot broadcast.
                let state = handle.state::<wiring::AppState>();
                let market = state.market.clone();
                let alerts = state.alerts.clone();
                let settings = state.settings.clone();

                // Background FX rate refresh. Fetches one daily candle for a handful
                // of FX pairs via the existing market provider chain (Yahoo handles
                // `=X` tickers under the `Forex` kind) and writes them into the
                // FxRateBook so portfolio valuation can aggregate across currencies.
                let fx_market = state.market.clone();
                let fx_book = state.fx.clone();
                let fx_emit = handle.clone();
                tauri::async_runtime::spawn(async move {
                    refresh_fx_rates(&fx_market, &fx_book, &fx_emit).await;
                    let mut ticker =
                        tokio::time::interval(std::time::Duration::from_secs(300));
                    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    // First tick fires immediately; we already did a pass, so skip it.
                    ticker.tick().await;
                    loop {
                        ticker.tick().await;
                        refresh_fx_rates(&fx_market, &fx_book, &fx_emit).await;
                    }
                });

                // Re-read the poll interval each tick so settings changes take effect
                // on the very next refresh without restarting the app.
                loop {
                    let interval_secs = settings
                        .get()
                        .await
                        .map(|s| s.poll_interval_secs)
                        .unwrap_or(5)
                        .clamp(1, 300) as u64;

                    match market.refresh().await {
                        Ok(outcome) => {
                            for err in &outcome.errors {
                                let _ = handle.emit("provider-error", err);
                            }
                            for q in &outcome.quotes {
                                let _ = alerts.evaluate_quote(q).await;
                            }
                            let snap_map = market.snapshot().await;
                            let dto: Vec<ipc::QuoteDto> = snap_map.values().map(|q| ipc::QuoteDto {
                                symbol: ipc::SymbolDto {
                                    kind: match q.symbol.kind() {
                                        domain::asset::AssetKind::Crypto => "crypto".into(),
                                        domain::asset::AssetKind::UsEquity => "us".into(),
                                        domain::asset::AssetKind::KrEquity => "kr".into(),
                                        domain::asset::AssetKind::Forex => "fx".into(),
                                        domain::asset::AssetKind::Commodity => "com".into(),
                                    },
                                    ticker: q.symbol.ticker().into(),
                                    quote_currency: q.symbol.quote_currency().map(|x| x.into()),
                                },
                                price: q.price.money().amount().to_string(),
                                currency: q.price.money().currency().as_str().to_string(),
                                change_24h: q.change_24h.map(|d| d.to_string()),
                                observed_at: q.observed_at.to_rfc3339(),
                                display_name: q.display_name.clone(),
                            }).collect();
                            let _ = handle.emit("quote-update", dto);
                        }
                        Err(e) => tracing::warn!(error = ?e, "refresh failed"),
                    }
                    tokio::time::sleep(Duration::from_secs(interval_secs)).await;
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Pull one daily candle per FX pair and write the latest close into the rate
/// book. `USDKRW=X` means "KRW per 1 USD", so the close goes into `(USD, KRW)`
/// directly; we also store the inverse so portfolio aggregation works in both
/// directions.
async fn refresh_fx_rates(
    market: &std::sync::Arc<application::market_service::MarketService>,
    fx: &application::fx_rate_book::FxRateBook,
    app: &tauri::AppHandle,
) {
    let pairs = [
        ("USDKRW=X", "USD", "KRW"),
        ("EURUSD=X", "EUR", "USD"),
        ("JPYUSD=X", "JPY", "USD"),
    ];
    let now = chrono::Utc::now();
    let from = now - chrono::Duration::days(3);
    for (yticker, from_code, to_code) in pairs {
        let Ok(from_c) = domain::money::Currency::new(from_code) else { continue };
        let Ok(to_c) = domain::money::Currency::new(to_code) else { continue };
        let symbol = match domain::symbol::Symbol::new(
            domain::asset::AssetKind::Forex,
            yticker,
            None,
        ) {
            Ok(s) => s,
            Err(_) => continue,
        };
        match market
            .fetch_candles(&symbol, from, now, domain::candle::CandleInterval::OneDay)
            .await
        {
            Ok(candles) if !candles.is_empty() => {
                let last = &candles[candles.len() - 1];
                let rate = last.close.money().amount();
                fx.set(from_c, to_c, rate).await;
                if rate > rust_decimal::Decimal::ZERO {
                    fx.set(to_c, from_c, rust_decimal::Decimal::ONE / rate).await;
                }
            }
            _ => {
                let _ = app.emit("fx-refresh-failed", format!("{yticker}: no candles"));
            }
        }
    }
}

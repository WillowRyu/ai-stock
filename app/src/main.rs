#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ipc;
mod tauri_notifier;
mod wiring;

use std::time::Duration;
use tauri::{Emitter, Manager};

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
            ipc::ai_set_key, ipc::ai_clear_key, ipc::ai_has_key, ipc::ai_commentary,
        ])
        .setup(|app| {
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

                // Periodic event emit loop (every 1s): refresh quotes, evaluate alerts,
                // broadcast snapshot to UI.
                let state = handle.state::<wiring::AppState>();
                loop {
                    let snap_map = state.market.snapshot().await;
                    let quotes: Vec<domain::quote::Quote> = snap_map.values().cloned().collect();
                    let alerts_svc = state.alerts.clone();
                    for q in &quotes {
                        let _ = alerts_svc.evaluate_quote(q).await;
                    }
                    let dto: Vec<ipc::QuoteDto> = quotes.iter().map(|q| ipc::QuoteDto {
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
                    }).collect();
                    let _ = handle.emit("quote-update", dto);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

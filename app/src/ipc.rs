use crate::wiring::AppState;
use application::indicator_service::compute_snapshot;
use application::ports::repos::AppSettings;
use domain::{
    asset::AssetKind, holding::Holding, money::{Currency, Money}, quantity::Quantity, symbol::Symbol,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tauri::State;

#[derive(Serialize, Deserialize, Clone)]
pub struct SymbolDto { pub kind: String, pub ticker: String, pub quote_currency: Option<String> }

#[derive(Serialize, Deserialize, Clone)]
pub struct QuoteDto {
    pub symbol: SymbolDto,
    pub price: String,
    pub currency: String,
    pub change_24h: Option<String>,
    pub observed_at: String,
}

fn kind_to_str(k: AssetKind) -> &'static str {
    match k {
        AssetKind::Crypto => "crypto", AssetKind::UsEquity => "us", AssetKind::KrEquity => "kr",
        AssetKind::Forex => "fx", AssetKind::Commodity => "com",
    }
}
fn str_to_kind(s: &str) -> Option<AssetKind> {
    Some(match s {
        "crypto" => AssetKind::Crypto, "us" => AssetKind::UsEquity, "kr" => AssetKind::KrEquity,
        "fx" => AssetKind::Forex, "com" => AssetKind::Commodity, _ => return None,
    })
}
fn dto_to_symbol(d: &SymbolDto) -> Result<Symbol, String> {
    let k = str_to_kind(&d.kind).ok_or_else(|| format!("bad kind: {}", d.kind))?;
    Symbol::new(k, &d.ticker, d.quote_currency.as_deref()).map_err(|e| format!("{e:?}"))
}
fn symbol_to_dto(s: &Symbol) -> SymbolDto {
    SymbolDto { kind: kind_to_str(s.kind()).into(), ticker: s.ticker().into(), quote_currency: s.quote_currency().map(|x| x.into()) }
}

#[tauri::command]
pub async fn watchlist_get(state: State<'_, AppState>) -> Result<Vec<SymbolDto>, String> {
    let wl = state.market.load_watchlist().await.map_err(|e| e.to_string())?;
    Ok(wl.symbols().iter().map(symbol_to_dto).collect())
}

#[tauri::command]
pub async fn watchlist_add(state: State<'_, AppState>, symbol: SymbolDto) -> Result<(), String> {
    let s = dto_to_symbol(&symbol)?;
    state.market.add_to_watchlist(s).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn watchlist_remove(state: State<'_, AppState>, symbol: SymbolDto) -> Result<(), String> {
    let s = dto_to_symbol(&symbol)?;
    state.market.remove_from_watchlist(&s).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn quotes_snapshot(state: State<'_, AppState>) -> Result<Vec<QuoteDto>, String> {
    let snap = state.market.snapshot().await;
    Ok(snap.values().map(|q| QuoteDto {
        symbol: symbol_to_dto(&q.symbol),
        price: q.price.money().amount().to_string(),
        currency: q.price.money().currency().as_str().to_string(),
        change_24h: q.change_24h.map(|d| d.to_string()),
        observed_at: q.observed_at.to_rfc3339(),
    }).collect())
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HoldingDto {
    pub symbol: SymbolDto,
    pub quantity: String,
    pub avg_cost_amount: String,
    pub avg_cost_currency: String,
}

#[tauri::command]
pub async fn portfolio_upsert(state: State<'_, AppState>, holding: HoldingDto) -> Result<(), String> {
    let symbol = dto_to_symbol(&holding.symbol)?;
    let qty = Quantity::new(Decimal::from_str(&holding.quantity).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{e:?}"))?;
    let ccy = Currency::new(&holding.avg_cost_currency).map_err(|e| format!("{e:?}"))?;
    let amt = Decimal::from_str(&holding.avg_cost_amount).map_err(|e| e.to_string())?;
    state.portfolio
        .upsert_holding(Holding::new(symbol, qty, Money::new(amt, ccy)))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn portfolio_delete(state: State<'_, AppState>, symbol: SymbolDto) -> Result<(), String> {
    let s = dto_to_symbol(&symbol)?;
    state.portfolio.delete_holding(&s).await.map_err(|e| e.to_string())
}

#[derive(Serialize, Clone)]
pub struct PortfolioValuationDto {
    pub total_value: Option<String>,
    pub total_value_currency: Option<String>,
    pub total_pnl: Option<String>,
    pub holdings: Vec<HoldingValuationDto>,
}

#[derive(Serialize, Clone)]
pub struct HoldingValuationDto {
    pub symbol: SymbolDto,
    pub market_value: Option<String>,
    pub cost_basis: String,
    pub pnl: Option<String>,
}

#[tauri::command]
pub async fn portfolio_valuation(state: State<'_, AppState>) -> Result<PortfolioValuationDto, String> {
    let v = state.portfolio.valuation().await.map_err(|e| e.to_string())?;
    Ok(PortfolioValuationDto {
        total_value: v.total_value.map(|m| m.amount().to_string()),
        total_value_currency: v.total_value.map(|m| m.currency().as_str().to_string()),
        total_pnl: v.total_pnl.map(|m| m.amount().to_string()),
        holdings: v.per_holding.iter().map(|h| HoldingValuationDto {
            symbol: symbol_to_dto(&h.symbol),
            market_value: h.market_value.map(|m| m.amount().to_string()),
            cost_basis: h.cost_basis.amount().to_string(),
            pnl: h.pnl_absolute.map(|m| m.amount().to_string()),
        }).collect(),
    })
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state.settings.get().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn settings_save(state: State<'_, AppState>, settings: AppSettings) -> Result<(), String> {
    state.settings.save(settings).await.map_err(|e| e.to_string())
}

#[derive(Serialize, Clone)]
pub struct IndicatorSnapshotDto {
    pub sma_20: Option<String>,
    pub sma_50: Option<String>,
    pub ema_20: Option<String>,
    pub rsi_14: Option<String>,
    pub macd: Option<String>,
    pub macd_signal: Option<String>,
    pub bollinger_upper: Option<String>,
    pub bollinger_lower: Option<String>,
}

#[tauri::command]
pub async fn indicators_for(
    state: State<'_, AppState>,
    symbol: SymbolDto,
    days: u32,
) -> Result<IndicatorSnapshotDto, String> {
    let s = dto_to_symbol(&symbol)?;
    let from = chrono::Utc::now() - chrono::Duration::days(days as i64);
    let to = chrono::Utc::now();
    let candles = state
        .market
        .fetch_candles(&s, from, to)
        .await
        .map_err(|e| e.to_string())?;
    let snap = compute_snapshot(&candles);
    let s = |d: Option<rust_decimal::Decimal>| d.map(|x| x.to_string());
    Ok(IndicatorSnapshotDto {
        sma_20: s(snap.sma_20),
        sma_50: s(snap.sma_50),
        ema_20: s(snap.ema_20),
        rsi_14: s(snap.rsi_14),
        macd: s(snap.macd),
        macd_signal: s(snap.macd_signal),
        bollinger_upper: s(snap.bollinger_upper),
        bollinger_lower: s(snap.bollinger_lower),
    })
}

#[tauri::command]
pub async fn widget_toggle(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = tauri::Manager::get_webview_window(&app, "widget") {
        if win.is_visible().unwrap_or(false) {
            win.hide().map_err(|e| e.to_string())?;
        } else {
            win.show().map_err(|e| e.to_string())?;
            win.set_focus().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

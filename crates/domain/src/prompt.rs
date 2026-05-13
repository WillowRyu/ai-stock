use crate::{quote::Quote, symbol::Symbol};
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct PromptContext<'a> {
    pub symbol: &'a Symbol,
    pub quote: Option<&'a Quote>,
    pub indicators: Option<IndicatorContext>,
    pub headlines: &'a [HeadlineRef<'a>],
}

#[derive(Debug, Clone, Copy)]
pub struct IndicatorContext {
    pub rsi_14: Option<Decimal>,
    pub macd: Option<Decimal>,
    pub macd_signal: Option<Decimal>,
    pub sma_20: Option<Decimal>,
    pub sma_50: Option<Decimal>,
}

#[derive(Debug, Clone, Copy)]
pub struct HeadlineRef<'a> {
    pub title: &'a str,
    pub source: &'a str,
}

pub fn build_commentary_prompt(ctx: &PromptContext) -> (String, String) {
    let system = "You are a concise financial-data assistant. Reply in 3-4 short sentences. Avoid jargon. Never give buy/sell advice. If data is missing, say so.".to_string();
    let mut user = format!("Asset: {} ({:?})\n", ctx.symbol.ticker(), ctx.symbol.kind());
    if let Some(q) = ctx.quote {
        user.push_str(&format!("Current price: {} {}\n", q.price.money().amount(), q.price.money().currency().as_str()));
        if let Some(c) = q.change_24h {
            user.push_str(&format!("24h change: {}%\n", c * Decimal::from(100)));
        }
    }
    if let Some(ind) = ctx.indicators {
        if let Some(rsi) = ind.rsi_14 { user.push_str(&format!("RSI(14): {}\n", rsi)); }
        if let (Some(m), Some(s)) = (ind.macd, ind.macd_signal) {
            user.push_str(&format!("MACD: {} (signal {})\n", m, s));
        }
        if let Some(sma20) = ind.sma_20 { user.push_str(&format!("SMA(20): {}\n", sma20)); }
        if let Some(sma50) = ind.sma_50 { user.push_str(&format!("SMA(50): {}\n", sma50)); }
    }
    if !ctx.headlines.is_empty() {
        user.push_str("Recent headlines:\n");
        for h in ctx.headlines {
            user.push_str(&format!("- {} ({})\n", h.title, h.source));
        }
    }
    user.push_str("\nBriefly summarize what's driving the recent price action and what the indicators are saying.");
    (system, user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        asset::AssetKind, money::{Currency, Money}, price::Price,
    };
    use chrono::Utc;
    use rust_decimal_macros::dec;

    #[test]
    fn includes_symbol_and_price_when_quote_provided() {
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        let q = Quote::new(s.clone(), Price::new(Money::new(dec!(67000), Currency::new("USD").unwrap())), Utc::now());
        let ctx = PromptContext { symbol: &s, quote: Some(&q), indicators: None, headlines: &[] };
        let (sys, user) = build_commentary_prompt(&ctx);
        assert!(sys.contains("financial-data assistant"));
        assert!(user.contains("BTC"));
        assert!(user.contains("67000"));
    }

    #[test]
    fn handles_missing_data_gracefully() {
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let ctx = PromptContext { symbol: &s, quote: None, indicators: None, headlines: &[] };
        let (_sys, user) = build_commentary_prompt(&ctx);
        assert!(user.contains("AAPL"));
    }
}

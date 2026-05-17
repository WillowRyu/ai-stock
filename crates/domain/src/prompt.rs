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

/// Which preset analysis a turn is. `Default` is `Commentary` so a
/// conversation started by a free-form message has a sensible persona.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptKind {
    #[default]
    Commentary,
    ChartAnalysis,
    NewsSummary,
}

impl PromptKind {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "commentary" => Self::Commentary,
            "chart_analysis" => Self::ChartAnalysis,
            "news_summary" => Self::NewsSummary,
            _ => return None,
        })
    }
}

/// The system prompt for a conversation of the given kind. Used both for the
/// first preset turn and for free-form follow-ups in the same conversation.
pub fn system_prompt(kind: PromptKind) -> String {
    match kind {
        PromptKind::Commentary => "You are a concise financial-data assistant. Reply in 3-4 short sentences. Avoid jargon. Never give buy/sell advice. If data is missing, say so.",
        PromptKind::ChartAnalysis => "You are a technical-analysis assistant. Interpret indicators objectively in 3-5 short sentences. Explain what RSI, MACD, and the moving averages suggest about momentum and trend. Never give buy/sell advice.",
        PromptKind::NewsSummary => "You are a financial-news summarizer. Summarize headlines neutrally in 3-4 short sentences, grouping related themes. Never give buy/sell advice. If there are no headlines, say so.",
    }
    .to_string()
}

/// The shared data dump (asset, price, indicators, headlines) that opens every
/// preset's first user message.
fn data_block(ctx: &PromptContext) -> String {
    let mut user = format!("Asset: {} ({:?})\n", ctx.symbol.ticker(), ctx.symbol.kind());
    if let Some(q) = ctx.quote {
        user.push_str(&format!(
            "Current price: {} {}\n",
            q.price.money().amount(),
            q.price.money().currency().as_str()
        ));
        if let Some(c) = q.change_24h {
            user.push_str(&format!("24h change: {}%\n", c * Decimal::from(100)));
        }
    }
    if let Some(ind) = ctx.indicators {
        if let Some(rsi) = ind.rsi_14 {
            user.push_str(&format!("RSI(14): {}\n", rsi));
        }
        if let (Some(m), Some(s)) = (ind.macd, ind.macd_signal) {
            user.push_str(&format!("MACD: {} (signal {})\n", m, s));
        }
        if let Some(sma20) = ind.sma_20 {
            user.push_str(&format!("SMA(20): {}\n", sma20));
        }
        if let Some(sma50) = ind.sma_50 {
            user.push_str(&format!("SMA(50): {}\n", sma50));
        }
    }
    if !ctx.headlines.is_empty() {
        user.push_str("Recent headlines:\n");
        for h in ctx.headlines {
            user.push_str(&format!("- {} ({})\n", h.title, h.source));
        }
    }
    user
}

pub fn build_commentary_prompt(ctx: &PromptContext) -> (String, String) {
    let mut user = data_block(ctx);
    user.push_str("\nBriefly summarize what's driving the recent price action and what the indicators are saying.");
    (system_prompt(PromptKind::Commentary), user)
}

pub fn build_chart_analysis_prompt(ctx: &PromptContext) -> (String, String) {
    let mut user = data_block(ctx);
    user.push_str("\nAnalyze the technical indicators above: interpret the RSI level, the MACD versus its signal line, and the SMA(20) versus SMA(50) relationship. Say what they jointly suggest about momentum and trend.");
    (system_prompt(PromptKind::ChartAnalysis), user)
}

pub fn build_news_summary_prompt(ctx: &PromptContext) -> (String, String) {
    let mut user = data_block(ctx);
    user.push_str("\nSummarize the key themes across the recent headlines above. If no headlines are present, say that no recent news was found.");
    (system_prompt(PromptKind::NewsSummary), user)
}

/// Dispatch to the builder for `kind`.
pub fn build_prompt(kind: PromptKind, ctx: &PromptContext) -> (String, String) {
    match kind {
        PromptKind::Commentary => build_commentary_prompt(ctx),
        PromptKind::ChartAnalysis => build_chart_analysis_prompt(ctx),
        PromptKind::NewsSummary => build_news_summary_prompt(ctx),
    }
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

    #[test]
    fn prompt_kind_parses_known_strings() {
        assert_eq!(PromptKind::parse("commentary"), Some(PromptKind::Commentary));
        assert_eq!(PromptKind::parse("chart_analysis"), Some(PromptKind::ChartAnalysis));
        assert_eq!(PromptKind::parse("news_summary"), Some(PromptKind::NewsSummary));
        assert_eq!(PromptKind::parse("bogus"), None);
    }

    #[test]
    fn chart_analysis_prompt_emphasizes_indicators() {
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let ind = IndicatorContext {
            rsi_14: Some(dec!(71)), macd: Some(dec!(1.2)), macd_signal: Some(dec!(0.9)),
            sma_20: Some(dec!(180)), sma_50: Some(dec!(175)),
        };
        let ctx = PromptContext { symbol: &s, quote: None, indicators: Some(ind), headlines: &[] };
        let (sys, user) = build_prompt(PromptKind::ChartAnalysis, &ctx);
        assert!(sys.contains("technical-analysis"));
        assert!(user.contains("RSI(14): 71"));
        assert!(user.contains("interpret the RSI"));
    }

    #[test]
    fn news_summary_prompt_emphasizes_headlines() {
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        let heads = [HeadlineRef { title: "ETF inflows surge", source: "Reuters" }];
        let ctx = PromptContext { symbol: &s, quote: None, indicators: None, headlines: &heads };
        let (sys, user) = build_prompt(PromptKind::NewsSummary, &ctx);
        assert!(sys.contains("news"));
        assert!(user.contains("ETF inflows surge"));
        assert!(user.contains("Summarize"));
    }

    #[test]
    fn build_prompt_commentary_matches_commentary_builder() {
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let ctx = PromptContext { symbol: &s, quote: None, indicators: None, headlines: &[] };
        let via_dispatch = build_prompt(PromptKind::Commentary, &ctx);
        let direct = build_commentary_prompt(&ctx);
        assert_eq!(via_dispatch, direct);
    }
}

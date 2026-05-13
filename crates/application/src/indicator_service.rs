use domain::{
    candle::Candle,
    indicators::{
        bollinger::{bollinger, BollingerOutput},
        ema::ema,
        macd::{macd, MacdOutput},
        rsi::rsi,
        sma::sma,
    },
};
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct IndicatorSnapshot {
    pub sma_20: Option<Decimal>,
    pub sma_50: Option<Decimal>,
    pub ema_20: Option<Decimal>,
    pub rsi_14: Option<Decimal>,
    pub macd: Option<Decimal>,
    pub macd_signal: Option<Decimal>,
    pub bollinger_upper: Option<Decimal>,
    pub bollinger_lower: Option<Decimal>,
}

pub fn compute_snapshot(candles: &[Candle]) -> IndicatorSnapshot {
    let closes: Vec<Decimal> = candles.iter().map(|c| c.close.money().amount()).collect();
    let last = |v: Vec<Option<Decimal>>| v.into_iter().last().flatten();
    let MacdOutput { macd: macd_vec, signal, .. } = macd(&closes, 12, 26, 9);
    let BollingerOutput { upper, lower, .. } = bollinger(&closes, 20, Decimal::from(2));
    IndicatorSnapshot {
        sma_20: last(sma(&closes, 20)),
        sma_50: last(sma(&closes, 50)),
        ema_20: last(ema(&closes, 20)),
        rsi_14: last(rsi(&closes, 14)),
        macd: last(macd_vec),
        macd_signal: last(signal),
        bollinger_upper: last(upper),
        bollinger_lower: last(lower),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::{
        asset::AssetKind,
        money::{Currency, Money},
        price::Price,
        symbol::Symbol,
    };
    use rust_decimal_macros::dec;

    #[test]
    fn snapshot_on_short_series_has_mostly_none() {
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        let p = |v| Price::new(Money::new(v, Currency::new("USD").unwrap()));
        let candles: Vec<Candle> = (1..=5)
            .map(|n| Candle {
                symbol: s.clone(),
                open: p(Decimal::from(n)),
                high: p(Decimal::from(n)),
                low: p(Decimal::from(n)),
                close: p(Decimal::from(n)),
                volume: dec!(0),
                opened_at: Utc::now(),
            })
            .collect();
        let snap = compute_snapshot(&candles);
        assert_eq!(snap.sma_50, None);
    }
}

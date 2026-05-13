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

#[derive(Debug, Clone, Serialize)]
pub struct IndicatorSeries {
    /// Indices align with the input `candles`. `None` entries fill the warm-up window.
    pub sma_20: Vec<Option<Decimal>>,
    pub sma_50: Vec<Option<Decimal>>,
    pub ema_20: Vec<Option<Decimal>>,
    pub rsi_14: Vec<Option<Decimal>>,
    pub macd: Vec<Option<Decimal>>,
    pub macd_signal: Vec<Option<Decimal>>,
    pub macd_histogram: Vec<Option<Decimal>>,
    pub bollinger_upper: Vec<Option<Decimal>>,
    pub bollinger_lower: Vec<Option<Decimal>>,
}

pub fn compute_series(candles: &[Candle]) -> IndicatorSeries {
    let closes: Vec<Decimal> = candles.iter().map(|c| c.close.money().amount()).collect();
    let MacdOutput { macd: m, signal, histogram } = macd(&closes, 12, 26, 9);
    let BollingerOutput { upper, lower, .. } = bollinger(&closes, 20, Decimal::from(2));
    IndicatorSeries {
        sma_20: sma(&closes, 20),
        sma_50: sma(&closes, 50),
        ema_20: ema(&closes, 20),
        rsi_14: rsi(&closes, 14),
        macd: m,
        macd_signal: signal,
        macd_histogram: histogram,
        bollinger_upper: upper,
        bollinger_lower: lower,
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

    #[test]
    fn series_lengths_match_candles_input() {
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let p = |v: Decimal| Price::new(Money::new(v, Currency::new("USD").unwrap()));
        let candles: Vec<Candle> = (1..=30)
            .map(|n| Candle {
                symbol: s.clone(),
                open: p(Decimal::from(n)),
                high: p(Decimal::from(n)),
                low: p(Decimal::from(n)),
                close: p(Decimal::from(n)),
                volume: Decimal::ZERO,
                opened_at: Utc::now(),
            })
            .collect();
        let series = compute_series(&candles);
        assert_eq!(series.sma_20.len(), 30);
        assert_eq!(series.rsi_14.len(), 30);
        // sma_50 is None throughout (only 30 candles, period=50)
        assert!(series.sma_50.iter().all(|x| x.is_none()));
        // sma_20 has Some from index 19 onward
        assert!(series.sma_20[19].is_some());
        assert!(series.sma_20[18].is_none());
    }
}

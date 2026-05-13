use super::ema::ema;
use rust_decimal::Decimal;

pub struct MacdOutput {
    pub macd: Vec<Option<Decimal>>,
    pub signal: Vec<Option<Decimal>>,
    pub histogram: Vec<Option<Decimal>>,
}

/// Classic MACD(12, 26, 9): fast EMA - slow EMA, plus 9-EMA of MACD as signal.
pub fn macd(closes: &[Decimal], fast: usize, slow: usize, signal_period: usize) -> MacdOutput {
    let fast_ema = ema(closes, fast);
    let slow_ema = ema(closes, slow);

    let macd: Vec<Option<Decimal>> = fast_ema
        .iter()
        .zip(slow_ema.iter())
        .map(|(f, s)| match (f, s) {
            (Some(f), Some(s)) => Some(*f - *s),
            _ => None,
        })
        .collect();

    let first_some = macd.iter().position(|x| x.is_some()).unwrap_or(macd.len());
    let dense: Vec<Decimal> = macd[first_some..].iter().filter_map(|x| *x).collect();
    let sig_dense = ema(&dense, signal_period);
    let mut signal: Vec<Option<Decimal>> = vec![None; first_some];
    signal.extend(sig_dense);
    signal.resize(macd.len(), None);

    let histogram: Vec<Option<Decimal>> = macd
        .iter()
        .zip(signal.iter())
        .map(|(m, s)| match (m, s) {
            (Some(m), Some(s)) => Some(*m - *s),
            _ => None,
        })
        .collect();

    MacdOutput { macd, signal, histogram }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn constant_series_is_zero() {
        let closes = vec![dec!(50); 60];
        let m = macd(&closes, 12, 26, 9);
        let last = m.macd.last().unwrap().unwrap();
        let sig = m.signal.last().unwrap().unwrap();
        assert_eq!(last, Decimal::ZERO);
        assert_eq!(sig, Decimal::ZERO);
    }
}

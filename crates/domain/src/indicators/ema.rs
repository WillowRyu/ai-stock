use rust_decimal::Decimal;

/// Exponential Moving Average with smoothing factor α = 2 / (period + 1).
/// Returns a vector of the same length as `closes`; the first `period - 1` entries are `None`.
/// The first EMA value at index `period - 1` is computed as the SMA of the first `period` closes,
/// then each subsequent value is `α * close + (1 - α) * prev`.
pub fn ema(closes: &[Decimal], period: usize) -> Vec<Option<Decimal>> {
    if period == 0 || closes.len() < period {
        return vec![None; closes.len()];
    }
    let alpha = Decimal::from(2) / Decimal::from((period as u64) + 1);
    let one_minus_alpha = Decimal::ONE - alpha;
    let mut out: Vec<Option<Decimal>> = vec![None; period - 1];

    let seed: Decimal = closes[..period].iter().sum::<Decimal>() / Decimal::from(period as u64);
    out.push(Some(seed));
    let mut prev = seed;
    for &c in &closes[period..] {
        let next = alpha * c + one_minus_alpha * prev;
        out.push(Some(next));
        prev = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn constant_series_is_flat() {
        let closes = vec![dec!(10); 10];
        let r = ema(&closes, 3);
        assert_eq!(r[2], Some(dec!(10)));
        assert_eq!(r[9], Some(dec!(10)));
    }

    #[test]
    fn short_series_returns_none() {
        assert_eq!(ema(&[dec!(1), dec!(2)], 5), vec![None, None]);
    }

    #[test]
    fn rising_series_ema_is_below_latest_close() {
        let closes: Vec<Decimal> = (1..=10).map(Decimal::from).collect();
        let r = ema(&closes, 3);
        let last = r.last().unwrap().unwrap();
        assert!(last < dec!(10));
        assert!(last > dec!(5));
    }
}

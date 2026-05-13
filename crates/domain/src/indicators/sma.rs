use rust_decimal::Decimal;

/// Simple Moving Average over `period` samples.
/// Returns a vector of the same length as `closes`; the first `period - 1` entries are `None`.
pub fn sma(closes: &[Decimal], period: usize) -> Vec<Option<Decimal>> {
    if period == 0 {
        return vec![None; closes.len()];
    }
    let mut out = Vec::with_capacity(closes.len());
    let mut sum = Decimal::ZERO;
    for (i, &c) in closes.iter().enumerate() {
        sum += c;
        if i >= period {
            sum -= closes[i - period];
        }
        if i + 1 >= period {
            out.push(Some(sum / Decimal::from(period as u64)));
        } else {
            out.push(None);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn period_three_matches_hand_computed() {
        let closes = vec![dec!(1), dec!(2), dec!(3), dec!(4), dec!(5)];
        let r = sma(&closes, 3);
        assert_eq!(r, vec![None, None, Some(dec!(2)), Some(dec!(3)), Some(dec!(4))]);
    }

    #[test]
    fn zero_period_returns_all_none() {
        let r = sma(&[dec!(1), dec!(2)], 0);
        assert_eq!(r, vec![None, None]);
    }

    #[test]
    fn period_longer_than_data_returns_all_none() {
        let r = sma(&[dec!(1), dec!(2)], 5);
        assert_eq!(r, vec![None, None]);
    }
}

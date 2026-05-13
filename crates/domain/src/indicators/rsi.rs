use rust_decimal::Decimal;

/// 14-period RSI by default (`period = 14`). Uses Wilder's smoothing.
/// Returns same-length vector; first `period` entries are `None`.
pub fn rsi(closes: &[Decimal], period: usize) -> Vec<Option<Decimal>> {
    if period == 0 || closes.len() <= period {
        return vec![None; closes.len()];
    }
    let mut out: Vec<Option<Decimal>> = vec![None; period];

    let (mut sum_gain, mut sum_loss) = (Decimal::ZERO, Decimal::ZERO);
    for w in closes.windows(2).take(period) {
        let diff = w[1] - w[0];
        if diff > Decimal::ZERO {
            sum_gain += diff;
        } else {
            sum_loss += -diff;
        }
    }
    let p = Decimal::from(period as u64);
    let mut avg_gain = sum_gain / p;
    let mut avg_loss = sum_loss / p;
    out.push(Some(rsi_from(avg_gain, avg_loss)));

    let p_minus_1 = p - Decimal::ONE;
    for w in closes.windows(2).skip(period) {
        let diff = w[1] - w[0];
        let (gain, loss) = if diff > Decimal::ZERO {
            (diff, Decimal::ZERO)
        } else {
            (Decimal::ZERO, -diff)
        };
        avg_gain = (avg_gain * p_minus_1 + gain) / p;
        avg_loss = (avg_loss * p_minus_1 + loss) / p;
        out.push(Some(rsi_from(avg_gain, avg_loss)));
    }
    out
}

fn rsi_from(avg_gain: Decimal, avg_loss: Decimal) -> Decimal {
    if avg_loss == Decimal::ZERO {
        return Decimal::from(100);
    }
    let rs = avg_gain / avg_loss;
    Decimal::from(100) - (Decimal::from(100) / (Decimal::ONE + rs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rust_decimal_macros::dec;

    #[test]
    fn monotonic_rising_pegs_at_100() {
        let closes: Vec<Decimal> = (1..=20).map(Decimal::from).collect();
        let r = rsi(&closes, 14);
        assert_eq!(r[14], Some(dec!(100)));
    }

    proptest! {
        #[test]
        fn rsi_in_zero_hundred(seed in -100i64..100, len in 16usize..50) {
            let closes: Vec<Decimal> = (0..len)
                .map(|i| Decimal::from(seed + i as i64))
                .collect();
            let r = rsi(&closes, 14);
            for v in r.into_iter().flatten() {
                prop_assert!(v >= Decimal::ZERO && v <= Decimal::from(100));
            }
        }
    }
}

use super::sma::sma;
use rust_decimal::Decimal;

pub struct BollingerOutput {
    pub middle: Vec<Option<Decimal>>,
    pub upper: Vec<Option<Decimal>>,
    pub lower: Vec<Option<Decimal>>,
}

/// `period`-SMA ± `k` * stddev. Default in finance literature: period=20, k=2.
pub fn bollinger(closes: &[Decimal], period: usize, k: Decimal) -> BollingerOutput {
    let middle = sma(closes, period);
    let mut upper = vec![None; closes.len()];
    let mut lower = vec![None; closes.len()];
    if period == 0 {
        return BollingerOutput { middle, upper, lower };
    }
    let p = Decimal::from(period as u64);

    for (i, m) in middle.iter().enumerate() {
        let Some(m) = m else { continue };
        let start = i + 1 - period;
        let mut var = Decimal::ZERO;
        for &c in &closes[start..=i] {
            let d = c - *m;
            var += d * d;
        }
        var /= p;
        let sd = sqrt_decimal(var);
        upper[i] = Some(*m + sd * k);
        lower[i] = Some(*m - sd * k);
    }
    BollingerOutput { middle, upper, lower }
}

/// Newton-Raphson sqrt for Decimal. Sufficient precision for indicator display (not for accounting).
fn sqrt_decimal(x: Decimal) -> Decimal {
    if x <= Decimal::ZERO {
        return Decimal::ZERO;
    }
    let mut guess = x;
    for _ in 0..32 {
        guess = (guess + x / guess) / Decimal::from(2);
    }
    guess
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn constant_series_has_zero_width() {
        let closes = vec![dec!(50); 25];
        let b = bollinger(&closes, 20, dec!(2));
        let upper = b.upper.last().unwrap().unwrap();
        let lower = b.lower.last().unwrap().unwrap();
        assert_eq!(upper, dec!(50));
        assert_eq!(lower, dec!(50));
    }
}

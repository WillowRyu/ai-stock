use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Percent(Decimal);

impl Percent {
    pub fn from_ratio(ratio: Decimal) -> Self {
        Self(ratio * Decimal::from(100))
    }
    pub fn from_value(v: Decimal) -> Self {
        Self(v)
    }
    pub fn value(&self) -> Decimal {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    #[test]
    fn ratio_to_percent() {
        assert_eq!(Percent::from_ratio(dec!(0.0124)).value(), dec!(1.2400));
    }
}

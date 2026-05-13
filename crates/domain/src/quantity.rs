use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Quantity(Decimal);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuantityError {
    #[error("quantity must be >= 0, got {0}")]
    Negative(Decimal),
}

impl Quantity {
    pub fn new(value: Decimal) -> Result<Self, QuantityError> {
        if value < Decimal::ZERO {
            return Err(QuantityError::Negative(value));
        }
        Ok(Self(value))
    }
    pub fn value(&self) -> Decimal {
        self.0
    }
    pub fn zero() -> Self {
        Self(Decimal::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn accepts_positive() {
        assert_eq!(Quantity::new(dec!(1.5)).unwrap().value(), dec!(1.5));
    }
    #[test]
    fn accepts_zero() {
        assert_eq!(Quantity::new(Decimal::ZERO).unwrap(), Quantity::zero());
    }
    #[test]
    fn rejects_negative() {
        assert!(matches!(Quantity::new(dec!(-0.1)), Err(QuantityError::Negative(_))));
    }
}

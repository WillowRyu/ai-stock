use rust_decimal::Decimal;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Currency([u8; 3]);

impl Currency {
    pub fn new(code: &str) -> Result<Self, MoneyError> {
        if code.len() != 3 || !code.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(MoneyError::InvalidCurrency(code.to_string()));
        }
        let bytes = code.as_bytes();
        Ok(Self([bytes[0], bytes[1], bytes[2]]))
    }
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Money {
    amount: Decimal,
    currency: Currency,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MoneyError {
    #[error("invalid currency code: {0}")]
    InvalidCurrency(String),
    #[error("currency mismatch: {0} vs {1}")]
    CurrencyMismatch(String, String),
    #[error("invalid amount: {0}")]
    InvalidAmount(String),
}

#[allow(clippy::should_implement_trait)] // `add`/`sub` return Result; can't implement std::ops traits.
impl Money {
    pub fn new(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }
    pub fn parse(amount: &str, currency: &str) -> Result<Self, MoneyError> {
        let amt = Decimal::from_str(amount).map_err(|_| MoneyError::InvalidAmount(amount.into()))?;
        Ok(Self { amount: amt, currency: Currency::new(currency)? })
    }
    pub fn amount(&self) -> Decimal { self.amount }
    pub fn currency(&self) -> Currency { self.currency }

    pub fn add(self, other: Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch(
                self.currency.as_str().into(),
                other.currency.as_str().into(),
            ));
        }
        Ok(Self { amount: self.amount + other.amount, currency: self.currency })
    }
    pub fn sub(self, other: Self) -> Result<Self, MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch(
                self.currency.as_str().into(),
                other.currency.as_str().into(),
            ));
        }
        Ok(Self { amount: self.amount - other.amount, currency: self.currency })
    }
    pub fn mul_scalar(self, factor: Decimal) -> Self {
        Self { amount: self.amount * factor, currency: self.currency }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parses_valid_money() {
        let m = Money::parse("12.50", "USD").unwrap();
        assert_eq!(m.amount(), dec!(12.50));
        assert_eq!(m.currency().as_str(), "USD");
    }

    #[test]
    fn rejects_lowercase_currency() {
        assert!(matches!(
            Money::parse("1", "usd"),
            Err(MoneyError::InvalidCurrency(_))
        ));
    }

    #[test]
    fn rejects_invalid_amount() {
        assert!(matches!(
            Money::parse("twelve", "USD"),
            Err(MoneyError::InvalidAmount(_))
        ));
    }

    #[test]
    fn adds_same_currency() {
        let a = Money::parse("10", "USD").unwrap();
        let b = Money::parse("2.5", "USD").unwrap();
        assert_eq!(a.add(b).unwrap(), Money::parse("12.5", "USD").unwrap());
    }

    #[test]
    fn rejects_cross_currency_addition() {
        let a = Money::parse("10", "USD").unwrap();
        let b = Money::parse("10", "KRW").unwrap();
        assert!(matches!(a.add(b), Err(MoneyError::CurrencyMismatch(_, _))));
    }

    #[test]
    fn multiplies_by_scalar() {
        let a = Money::parse("3", "USD").unwrap();
        assert_eq!(a.mul_scalar(dec!(2)), Money::parse("6", "USD").unwrap());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn addition_is_commutative(a in -1_000_000i64..1_000_000, b in -1_000_000i64..1_000_000) {
            let m1 = Money::new(Decimal::from(a), Currency::new("USD").unwrap());
            let m2 = Money::new(Decimal::from(b), Currency::new("USD").unwrap());
            prop_assert_eq!(m1.add(m2).unwrap(), m2.add(m1).unwrap());
        }
    }
}

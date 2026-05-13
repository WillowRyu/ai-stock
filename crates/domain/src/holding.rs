use crate::{money::Money, quantity::Quantity, symbol::Symbol};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Holding {
    pub symbol: Symbol,
    pub quantity: Quantity,
    pub avg_cost: Money, // per-unit cost basis in the holding's quote currency
}

impl Holding {
    pub fn new(symbol: Symbol, quantity: Quantity, avg_cost: Money) -> Self {
        Self { symbol, quantity, avg_cost }
    }
    pub fn cost_basis(&self) -> Money {
        self.avg_cost.mul_scalar(self.quantity.value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetKind, money::Currency};
    use rust_decimal_macros::dec;

    #[test]
    fn computes_cost_basis() {
        let h = Holding::new(
            Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap(),
            Quantity::new(dec!(10)).unwrap(),
            Money::new(dec!(150), Currency::new("USD").unwrap()),
        );
        assert_eq!(h.cost_basis(), Money::new(dec!(1500), Currency::new("USD").unwrap()));
    }
}

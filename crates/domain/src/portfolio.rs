use crate::holding::Holding;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Portfolio {
    holdings: Vec<Holding>,
}

impl Portfolio {
    pub fn new() -> Self { Self::default() }
    pub fn holdings(&self) -> &[Holding] { &self.holdings }
    pub fn upsert(&mut self, h: Holding) {
        if let Some(existing) = self.holdings.iter_mut().find(|x| x.symbol == h.symbol) {
            *existing = h;
        } else {
            self.holdings.push(h);
        }
    }
    pub fn remove(&mut self, symbol: &crate::symbol::Symbol) -> bool {
        let len = self.holdings.len();
        self.holdings.retain(|x| &x.symbol != symbol);
        self.holdings.len() != len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{asset::AssetKind, money::{Currency, Money}, quantity::Quantity, symbol::Symbol};
    use rust_decimal_macros::dec;
    #[test]
    fn upsert_replaces_existing() {
        let mut p = Portfolio::new();
        let s = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let h1 = Holding::new(s.clone(), Quantity::new(dec!(10)).unwrap(), Money::new(dec!(100), Currency::new("USD").unwrap()));
        let h2 = Holding::new(s.clone(), Quantity::new(dec!(20)).unwrap(), Money::new(dec!(110), Currency::new("USD").unwrap()));
        p.upsert(h1);
        p.upsert(h2);
        assert_eq!(p.holdings().len(), 1);
        assert_eq!(p.holdings()[0].quantity.value(), dec!(20));
    }
}

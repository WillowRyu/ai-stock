use crate::money::Money;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Price(Money);

impl Price {
    pub fn new(money: Money) -> Self {
        Self(money)
    }
    pub fn money(&self) -> Money {
        self.0
    }
}

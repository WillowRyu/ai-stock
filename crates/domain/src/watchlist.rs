use crate::symbol::Symbol;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Watchlist {
    symbols: Vec<Symbol>,
}

impl Watchlist {
    pub fn new() -> Self { Self::default() }
    pub fn symbols(&self) -> &[Symbol] { &self.symbols }
    pub fn add(&mut self, s: Symbol) -> bool {
        if self.symbols.contains(&s) { return false; }
        self.symbols.push(s);
        true
    }
    pub fn remove(&mut self, s: &Symbol) -> bool {
        let len = self.symbols.len();
        self.symbols.retain(|x| x != s);
        self.symbols.len() != len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::AssetKind;
    #[test]
    fn add_is_idempotent() {
        let mut w = Watchlist::new();
        let s = Symbol::new(AssetKind::Crypto, "BTC", Some("USD")).unwrap();
        assert!(w.add(s.clone()));
        assert!(!w.add(s));
        assert_eq!(w.symbols().len(), 1);
    }
}

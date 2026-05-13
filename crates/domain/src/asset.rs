#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AssetKind {
    Crypto,
    UsEquity,
    KrEquity,
    Forex,
    Commodity,
}

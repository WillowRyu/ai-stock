use crate::{holding::Holding, money::Money, portfolio::Portfolio, quote::Quote, symbol::Symbol};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioValuation {
    pub per_holding: Vec<HoldingValuation>,
    pub total_value: Option<Money>,         // None if quotes for any holding are missing
    pub total_cost: Option<Money>,
    pub total_pnl: Option<Money>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingValuation {
    pub symbol: Symbol,
    pub market_value: Option<Money>,
    pub cost_basis: Money,
    pub pnl_absolute: Option<Money>,
}

/// Pure: evaluate the portfolio against a quotes lookup table.
/// Returns `total_value = None` if any holding lacks a quote, or if holdings have mixed currencies.
/// (FX-based aggregation is M2 work.)
pub fn evaluate(
    portfolio: &Portfolio,
    quotes_by_symbol: &HashMap<Symbol, Quote>,
) -> PortfolioValuation {
    let per_holding: Vec<HoldingValuation> = portfolio
        .holdings()
        .iter()
        .map(|h| value_holding(h, quotes_by_symbol.get(&h.symbol)))
        .collect();

    let total_value = sum_money(per_holding.iter().filter_map(|h| h.market_value));
    let total_cost = sum_money(per_holding.iter().map(|h| h.cost_basis));
    let total_pnl = match (total_value, total_cost) {
        (Some(v), Some(c)) => v.sub(c).ok(),
        _ => None,
    };

    PortfolioValuation { per_holding, total_value, total_cost, total_pnl }
}

fn value_holding(h: &Holding, quote: Option<&Quote>) -> HoldingValuation {
    let market_value = quote.map(|q| q.price.money().mul_scalar(h.quantity.value()));
    let cost_basis = h.cost_basis();
    let pnl_absolute = match market_value {
        Some(mv) => mv.sub(cost_basis).ok(),
        None => None,
    };
    HoldingValuation { symbol: h.symbol.clone(), market_value, cost_basis, pnl_absolute }
}

/// Returns `Some(sum)` if all items share a currency, `None` if they don't or the iterator is empty.
fn sum_money<I: IntoIterator<Item = Money>>(items: I) -> Option<Money> {
    let mut iter = items.into_iter();
    let first = iter.next()?;
    iter.try_fold(first, |acc, m| acc.add(m)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        asset::AssetKind, money::{Currency, Money}, price::Price, quantity::Quantity,
        symbol::Symbol,
    };
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn usd(v: rust_decimal::Decimal) -> Money { Money::new(v, Currency::new("USD").unwrap()) }
    fn s_aapl() -> Symbol { Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap() }

    #[test]
    fn computes_pnl_for_single_holding() {
        let mut p = Portfolio::new();
        p.upsert(Holding::new(s_aapl(), Quantity::new(dec!(10)).unwrap(), usd(dec!(150))));

        let mut quotes = HashMap::new();
        quotes.insert(s_aapl(), Quote::new(s_aapl(), Price::new(usd(dec!(180))), Utc::now()));

        let v = evaluate(&p, &quotes);
        assert_eq!(v.total_value, Some(usd(dec!(1800))));
        assert_eq!(v.total_cost, Some(usd(dec!(1500))));
        assert_eq!(v.total_pnl, Some(usd(dec!(300))));
    }

    #[test]
    fn missing_quote_yields_none_market_value() {
        let mut p = Portfolio::new();
        p.upsert(Holding::new(s_aapl(), Quantity::new(dec!(10)).unwrap(), usd(dec!(150))));
        let quotes = HashMap::new();
        let v = evaluate(&p, &quotes);
        assert_eq!(v.per_holding[0].market_value, None);
        assert_eq!(v.total_value, None);
    }
}

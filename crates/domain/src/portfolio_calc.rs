use crate::{fx::FxRates, holding::Holding, money::{Currency, Money}, portfolio::Portfolio, quote::Quote, symbol::Symbol};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioValuation {
    pub per_holding: Vec<HoldingValuation>,
    pub total_value: Option<Money>,
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

/// Evaluate the portfolio. Each holding's `market_value` and `cost_basis` is
/// returned in its own native currency for display; the totals are aggregated
/// in `display_currency` after converting via `fx_rates`. If any conversion
/// is missing, the corresponding total is `None`.
pub fn evaluate(
    portfolio: &Portfolio,
    quotes_by_symbol: &HashMap<Symbol, Quote>,
    fx_rates: &FxRates,
    display_currency: Currency,
) -> PortfolioValuation {
    let per_holding: Vec<HoldingValuation> = portfolio
        .holdings()
        .iter()
        .map(|h| value_holding(h, quotes_by_symbol.get(&h.symbol)))
        .collect();

    let total_value = sum_in(
        per_holding.iter().filter_map(|h| h.market_value),
        fx_rates,
        display_currency,
    );
    let total_cost = sum_in(
        per_holding.iter().map(|h| h.cost_basis),
        fx_rates,
        display_currency,
    );
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
        Some(mv) if mv.currency() == cost_basis.currency() => mv.sub(cost_basis).ok(),
        _ => None,
    };
    HoldingValuation { symbol: h.symbol.clone(), market_value, cost_basis, pnl_absolute }
}

/// Convert every item into `target` and sum. Returns `None` if any conversion
/// fails (missing FX rate) or the input is empty.
fn sum_in<I: IntoIterator<Item = Money>>(items: I, fx: &FxRates, target: Currency) -> Option<Money> {
    let mut iter = items.into_iter();
    let first = fx.convert(iter.next()?, target)?;
    iter.try_fold(first, |acc, m| {
        let converted = fx.convert(m, target)?;
        acc.add(converted).ok()
    })
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

        let fx = FxRates::new();
        let v = evaluate(&p, &quotes, &fx, Currency::new("USD").unwrap());
        assert_eq!(v.total_value, Some(usd(dec!(1800))));
        assert_eq!(v.total_cost, Some(usd(dec!(1500))));
        assert_eq!(v.total_pnl, Some(usd(dec!(300))));
    }

    #[test]
    fn missing_quote_yields_none_market_value() {
        let mut p = Portfolio::new();
        p.upsert(Holding::new(s_aapl(), Quantity::new(dec!(10)).unwrap(), usd(dec!(150))));
        let quotes = HashMap::new();
        let fx = FxRates::new();
        let v = evaluate(&p, &quotes, &fx, Currency::new("USD").unwrap());
        assert_eq!(v.per_holding[0].market_value, None);
        assert_eq!(v.total_value, None);
    }

    #[test]
    fn aggregates_across_currencies_via_fx() {
        // 10 AAPL at $150 + 100 samsung at ₩70000, display in USD with 1 USD = 1400 KRW.
        use crate::asset::AssetKind;
        let s_aapl = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let s_005930 = Symbol::new(AssetKind::KrEquity, "005930", None).unwrap();
        let krw = |v| Money::new(v, Currency::new("KRW").unwrap());

        let mut p = Portfolio::new();
        p.upsert(Holding::new(s_aapl.clone(), Quantity::new(dec!(10)).unwrap(), usd(dec!(150))));
        p.upsert(Holding::new(s_005930.clone(), Quantity::new(dec!(100)).unwrap(), krw(dec!(70000))));

        let mut quotes = HashMap::new();
        quotes.insert(s_aapl.clone(), Quote::new(s_aapl, Price::new(usd(dec!(180))), Utc::now()));
        quotes.insert(s_005930.clone(), Quote::new(s_005930, Price::new(krw(dec!(75000))), Utc::now()));

        let mut fx = FxRates::new();
        fx.set(Currency::new("KRW").unwrap(), Currency::new("USD").unwrap(), dec!(0.000714286));
        // (1/1400) — close enough for an integer-driven assertion below.

        let v = evaluate(&p, &quotes, &fx, Currency::new("USD").unwrap());
        // AAPL: 10 × 180 = $1800.  KRW position: 100 × 75000 = ₩7,500,000 ≈ $5357.14
        // Total ≈ $7157.14. We don't assert the exact decimal — just both signs/order.
        let total = v.total_value.unwrap();
        assert_eq!(total.currency().as_str(), "USD");
        assert!(total.amount() > dec!(7000));
        assert!(total.amount() < dec!(7300));
    }

    #[test]
    fn aggregation_returns_none_when_fx_missing() {
        use crate::asset::AssetKind;
        let s_aapl = Symbol::new(AssetKind::UsEquity, "AAPL", None).unwrap();
        let s_005930 = Symbol::new(AssetKind::KrEquity, "005930", None).unwrap();
        let krw = |v| Money::new(v, Currency::new("KRW").unwrap());

        let mut p = Portfolio::new();
        p.upsert(Holding::new(s_aapl.clone(), Quantity::new(dec!(10)).unwrap(), usd(dec!(150))));
        p.upsert(Holding::new(s_005930.clone(), Quantity::new(dec!(100)).unwrap(), krw(dec!(70000))));

        let mut quotes = HashMap::new();
        quotes.insert(s_aapl.clone(), Quote::new(s_aapl, Price::new(usd(dec!(180))), Utc::now()));
        quotes.insert(s_005930.clone(), Quote::new(s_005930, Price::new(krw(dec!(75000))), Utc::now()));

        let fx = FxRates::new(); // empty — no KRW→USD rate
        let v = evaluate(&p, &quotes, &fx, Currency::new("USD").unwrap());
        assert_eq!(v.total_value, None);
    }
}

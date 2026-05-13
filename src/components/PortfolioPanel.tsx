import { useEffect, useState } from "react";
import { usePortfolioStore } from "../lib/state/portfolioStore";
import type { AssetKind, HoldingDto } from "../lib/ipc";

export function PortfolioPanel() {
  const { valuation, refresh, upsert, remove } = usePortfolioStore();
  const [open, setOpen] = useState(false);

  useEffect(() => { refresh(); }, [refresh]);

  return (
    <aside className="w-80 border-l border-slate-800 flex flex-col">
      <div className="p-3 border-b border-slate-800 flex justify-between items-center">
        <span className="text-xs uppercase text-slate-400">Portfolio</span>
        <button onClick={() => setOpen(true)} className="text-xs px-2 py-1 rounded bg-slate-800 hover:bg-slate-700">+ Add</button>
      </div>

      <div className="p-3 border-b border-slate-800">
        <div className="text-xs text-slate-500">총 평가액</div>
        <div className="text-xl tabular-nums">
          {valuation?.total_value ?? "—"} {valuation?.total_value_currency ?? ""}
        </div>
        <div className={"text-xs " + ((Number(valuation?.total_pnl ?? "0") >= 0) ? "text-emerald-400" : "text-rose-400")}>
          P&L: {valuation?.total_pnl ?? "—"}
        </div>
      </div>

      <ul className="flex-1 overflow-y-auto text-xs">
        {valuation?.holdings.map((h, i) => (
          <li key={i} className="p-2 border-b border-slate-900 flex justify-between">
            <div>
              <div>{h.symbol.ticker}</div>
              <div className="text-slate-500">cost: {h.cost_basis}</div>
            </div>
            <div className="text-right">
              <div>{h.market_value ?? "—"}</div>
              <div className={(Number(h.pnl ?? "0") >= 0) ? "text-emerald-400" : "text-rose-400"}>
                {h.pnl ?? "—"}
              </div>
            </div>
            <button onClick={() => remove(h.symbol)} className="ml-2 text-slate-600 hover:text-rose-400">×</button>
          </li>
        ))}
      </ul>

      {open && <AddHoldingDialog onClose={() => setOpen(false)} onSubmit={upsert} />}
    </aside>
  );
}

function AddHoldingDialog({ onClose, onSubmit }: { onClose(): void; onSubmit(h: HoldingDto): Promise<void> }) {
  const [kind, setKind] = useState<AssetKind>("crypto");
  const [ticker, setTicker] = useState("BTC");
  const [quote, setQuote] = useState("USD");
  const [qty, setQty] = useState("0");
  const [cost, setCost] = useState("0");
  const [ccy, setCcy] = useState("USD");
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      await onSubmit({
        symbol: { kind, ticker: ticker.toUpperCase(), quote_currency: kind === "crypto" ? quote.toUpperCase() : null },
        quantity: qty, avg_cost_amount: cost, avg_cost_currency: ccy.toUpperCase(),
      });
      onClose();
    } catch (err) { setError(String(err)); }
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <form onClick={(e) => e.stopPropagation()} onSubmit={submit}
            className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-96 space-y-3">
        <h3 className="text-lg font-semibold">보유 자산 추가</h3>
        <select value={kind} onChange={(e) => setKind(e.target.value as AssetKind)} className="w-full bg-slate-800 rounded p-1.5">
          <option value="crypto">Crypto</option>
          <option value="us">US Equity</option>
        </select>
        <input value={ticker} onChange={(e) => setTicker(e.target.value)} placeholder="ticker" className="w-full bg-slate-800 rounded p-1.5" />
        {kind === "crypto" && (
          <input value={quote} onChange={(e) => setQuote(e.target.value)} placeholder="quote currency" className="w-full bg-slate-800 rounded p-1.5" />
        )}
        <input value={qty} onChange={(e) => setQty(e.target.value)} placeholder="수량" className="w-full bg-slate-800 rounded p-1.5" />
        <input value={cost} onChange={(e) => setCost(e.target.value)} placeholder="평단가" className="w-full bg-slate-800 rounded p-1.5" />
        <input value={ccy} onChange={(e) => setCcy(e.target.value)} placeholder="통화" className="w-full bg-slate-800 rounded p-1.5" />
        {error && <div className="text-rose-400 text-xs">{error}</div>}
        <div className="flex gap-2 justify-end">
          <button type="button" onClick={onClose} className="px-3 py-1 text-sm rounded bg-slate-800">취소</button>
          <button type="submit" className="px-3 py-1 text-sm rounded bg-emerald-600">저장</button>
        </div>
      </form>
    </div>
  );
}

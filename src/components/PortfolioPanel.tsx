import { useEffect, useMemo, useState } from "react";
import { usePortfolioStore } from "../lib/state/portfolioStore";
import { useWatchlistStore } from "../lib/state/watchlistStore";
import { formatMoney } from "../lib/format";
import type { HoldingDto, SymbolDto } from "../lib/ipc";

function defaultCostCurrency(s: SymbolDto): string {
  // For crypto we have the quote currency (USDT/USD/...) attached to the symbol.
  if (s.quote_currency) return s.quote_currency;
  switch (s.kind) {
    case "us": return "USD";
    case "kr": return "KRW";
    case "fx":
    case "com":
    default:
      return "USD";
  }
}

function symbolLabel(s: SymbolDto): string {
  return s.quote_currency ? `${s.ticker} / ${s.quote_currency}` : s.ticker;
}

function symbolKey(s: SymbolDto): string {
  return s.quote_currency ? `${s.kind}:${s.ticker}:${s.quote_currency}` : `${s.kind}:${s.ticker}`;
}

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
          {valuation?.total_value ? formatMoney(valuation.total_value) : "—"} {valuation?.total_value_currency ?? ""}
        </div>
        <div className={"text-xs " + ((Number(valuation?.total_pnl ?? "0") >= 0) ? "text-emerald-400" : "text-rose-400")}>
          P&L: {valuation?.total_pnl ? formatMoney(valuation.total_pnl) : "—"}
        </div>
      </div>

      {(!valuation || valuation.holdings.length === 0) ? (
        <div className="flex-1 flex flex-col items-center justify-center text-center text-xs text-slate-500 px-4 py-8 gap-2">
          <p>보유 자산을 입력하면 실시간 평가/손익이 표시됩니다</p>
          <button
            onClick={() => setOpen(true)}
            className="px-3 py-1.5 bg-emerald-600 hover:bg-emerald-500 rounded text-slate-50"
          >
            + 자산 추가
          </button>
        </div>
      ) : (
        <ul className="flex-1 overflow-y-auto text-xs">
          {valuation.holdings.map((h, i) => (
            <li key={i} className="p-2 border-b border-slate-900 flex justify-between">
              <div>
                <div>{h.symbol.ticker}</div>
                <div className="text-slate-500">cost: {formatMoney(h.cost_basis)}</div>
              </div>
              <div className="text-right">
                <div>{h.market_value ? formatMoney(h.market_value) : "—"}</div>
                <div className={(Number(h.pnl ?? "0") >= 0) ? "text-emerald-400" : "text-rose-400"}>
                  {h.pnl ? formatMoney(h.pnl) : "—"}
                </div>
              </div>
              <button onClick={() => remove(h.symbol)} className="ml-2 text-slate-600 hover:text-rose-400">×</button>
            </li>
          ))}
        </ul>
      )}

      {open && <AddHoldingDialog onClose={() => setOpen(false)} onSubmit={upsert} />}
    </aside>
  );
}

function AddHoldingDialog({ onClose, onSubmit }: { onClose(): void; onSubmit(h: HoldingDto): Promise<void> }) {
  const watchlist = useWatchlistStore((s) => s.symbols);
  const loadWatchlist = useWatchlistStore((s) => s.load);
  // Make sure the watchlist is loaded when this dialog mounts (it might not be
  // populated if the user opened the portfolio panel before the watchlist).
  useEffect(() => {
    if (watchlist.length === 0) loadWatchlist();
  }, [watchlist.length, loadWatchlist]);

  const [selectedKey, setSelectedKey] = useState<string>(() =>
    watchlist[0] ? symbolKey(watchlist[0]) : "",
  );
  const [qty, setQty] = useState("");
  const [cost, setCost] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Keep selection valid when watchlist becomes available.
  useEffect(() => {
    if (!selectedKey && watchlist[0]) setSelectedKey(symbolKey(watchlist[0]));
  }, [selectedKey, watchlist]);

  const selectedSymbol = useMemo<SymbolDto | undefined>(
    () => watchlist.find((s) => symbolKey(s) === selectedKey),
    [watchlist, selectedKey],
  );
  const costCurrency = selectedSymbol ? defaultCostCurrency(selectedSymbol) : "USD";

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (!selectedSymbol) {
      setError("종목을 선택하세요");
      return;
    }
    if (!qty || Number(qty) <= 0) {
      setError("수량을 0보다 크게 입력하세요");
      return;
    }
    if (!cost || Number(cost) <= 0) {
      setError("평단가를 0보다 크게 입력하세요");
      return;
    }
    setBusy(true);
    try {
      await onSubmit({
        symbol: selectedSymbol,
        quantity: qty,
        avg_cost_amount: cost,
        avg_cost_currency: costCurrency,
      });
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <form
        onClick={(e) => e.stopPropagation()}
        onSubmit={submit}
        className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-[28rem] space-y-4"
      >
        <h3 className="text-lg font-semibold">보유 자산 추가</h3>

        {watchlist.length === 0 ? (
          <p className="text-sm text-slate-400">
            먼저 좌측 워치리스트에 종목을 추가해 주세요. 추가된 종목 중에서 골라 보유량과 평단가를
            입력하면 실시간 평가가 시작됩니다.
          </p>
        ) : (
          <>
            <label className="block text-sm">
              <span className="text-slate-300">종목</span>
              <select
                value={selectedKey}
                onChange={(e) => setSelectedKey(e.target.value)}
                className="mt-1 w-full bg-slate-800 rounded px-3 py-2.5 text-base"
              >
                {watchlist.map((s) => (
                  <option key={symbolKey(s)} value={symbolKey(s)}>
                    {symbolLabel(s)}
                  </option>
                ))}
              </select>
            </label>

            <label className="block text-sm">
              <span className="text-slate-300">보유 수량</span>
              <input
                value={qty}
                onChange={(e) => setQty(e.target.value)}
                inputMode="decimal"
                placeholder="예: 0.5"
                className="mt-1 w-full bg-slate-800 rounded px-3 py-2.5 text-base"
              />
            </label>

            <label className="block text-sm">
              <span className="text-slate-300">평단가 ({costCurrency})</span>
              <input
                value={cost}
                onChange={(e) => setCost(e.target.value)}
                inputMode="decimal"
                placeholder="1주/1개당 평균 매입가"
                className="mt-1 w-full bg-slate-800 rounded px-3 py-2.5 text-base"
              />
            </label>
          </>
        )}

        {error && <div className="text-rose-400 text-xs">{error}</div>}
        <div className="flex gap-2 justify-end">
          <button type="button" onClick={onClose} className="px-3 py-1.5 text-sm rounded bg-slate-800 hover:bg-slate-700">취소</button>
          <button
            type="submit"
            disabled={busy || watchlist.length === 0}
            className="px-3 py-1.5 text-sm rounded bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50"
          >
            {busy ? "저장 중..." : "저장"}
          </button>
        </div>
      </form>
    </div>
  );
}

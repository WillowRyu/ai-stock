import { useQuotesStore, quoteKey } from "../lib/state/quotesStore";
import type { SymbolDto } from "../lib/ipc";

export function DetailPane({ symbol }: { symbol: SymbolDto | null }) {
  const quotes = useQuotesStore((s) => s.bySymbol);
  if (!symbol) {
    return <div className="flex-1 flex items-center justify-center text-slate-500 text-sm">워치리스트에서 종목을 선택하세요</div>;
  }
  const q = quotes[quoteKey(symbol)];
  const changePct = q?.change_24h ? Number(q.change_24h) * 100 : null;
  return (
    <main className="flex-1 p-6">
      <div className="text-xs uppercase text-slate-500">{symbol.kind}</div>
      <h2 className="text-2xl font-semibold">{symbol.ticker}{symbol.quote_currency ? `/${symbol.quote_currency}` : ""}</h2>
      <div className="mt-4 flex items-baseline gap-3">
        <div className="text-4xl tabular-nums">{q?.price ?? "—"}</div>
        <div className="text-slate-400">{q?.currency ?? ""}</div>
        {changePct !== null && (
          <div className={changePct >= 0 ? "text-emerald-400" : "text-rose-400"}>
            {changePct >= 0 ? "+" : ""}{changePct.toFixed(2)}%
          </div>
        )}
      </div>
      <p className="mt-2 text-xs text-slate-500">Last observed: {q?.observed_at ?? "—"}</p>
    </main>
  );
}

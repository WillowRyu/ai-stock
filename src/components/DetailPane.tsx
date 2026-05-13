import { useEffect, useState } from "react";
import { useQuotesStore, quoteKey } from "../lib/state/quotesStore";
import { formatPrice } from "../lib/format";
import { formatObservedAt, getDefaultTimezone, getTimezoneOptions } from "../lib/timezone";
import { ChartPanel } from "./ChartPanel";
import type { SymbolDto } from "../lib/ipc";

export function DetailPane({ symbol }: { symbol: SymbolDto | null }) {
  if (!symbol) {
    return <div className="flex-1 flex items-center justify-center text-slate-500 text-sm">워치리스트에서 종목을 선택하세요</div>;
  }
  return <SymbolDetail symbol={symbol} />;
}

function SymbolDetail({ symbol }: { symbol: SymbolDto }) {
  const quotes = useQuotesStore((s) => s.bySymbol);
  const q = quotes[quoteKey(symbol)];
  const changePct = q?.change_24h ? Number(q.change_24h) * 100 : null;

  // Timezone: defaults from asset kind (US stocks → New York, KR → Seoul,
  // crypto/fx/commodity → user's local). Reset to the default whenever the
  // selected symbol's kind changes; user can still override via the dropdown.
  const [tz, setTz] = useState<string>(() => getDefaultTimezone(symbol.kind));
  useEffect(() => {
    setTz(getDefaultTimezone(symbol.kind));
  }, [symbol.kind]);
  const tzOptions = getTimezoneOptions();

  return (
    <main className="flex-1 p-4 overflow-y-auto">
      <div className="text-xs uppercase text-slate-500">{symbol.kind}</div>
      <h2 className="text-2xl font-semibold">{symbol.ticker}{symbol.quote_currency ? `/${symbol.quote_currency}` : ""}</h2>
      <div className="mt-2 flex items-baseline gap-3">
        <div className="text-4xl tabular-nums">{q ? formatPrice(q.price) : "—"}</div>
        <div className="text-slate-400">{q?.currency ?? ""}</div>
        {changePct !== null && (
          <div className={changePct >= 0 ? "text-emerald-400" : "text-rose-400"}>
            {changePct >= 0 ? "+" : ""}{changePct.toFixed(2)}%
          </div>
        )}
      </div>
      <div className="mt-1 mb-3 flex items-center gap-2 text-xs text-slate-500">
        <span>Last observed</span>
        <span className="text-slate-300 tabular-nums">{q ? formatObservedAt(q.observed_at, tz) : "—"}</span>
        <select
          value={tz}
          onChange={(e) => setTz(e.target.value)}
          className="bg-slate-800 rounded px-1.5 py-0.5 text-[10px] text-slate-200"
          title="표시 시간대"
        >
          {tzOptions.map((o) => (
            <option key={o.value} value={o.value}>{o.label}</option>
          ))}
        </select>
      </div>
      <ChartPanel symbol={symbol} />
    </main>
  );
}

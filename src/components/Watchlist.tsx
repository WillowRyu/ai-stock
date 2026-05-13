import { useEffect, useState } from "react";
import clsx from "clsx";
import { useWatchlistStore } from "../lib/state/watchlistStore";
import { useQuotesStore, quoteKey } from "../lib/state/quotesStore";
import { formatPrice } from "../lib/format";
import type { SymbolDto } from "../lib/ipc";

interface Props {
  selected: SymbolDto | null;
  onSelect(s: SymbolDto): void;
  onAdd(): void;
}

export function Watchlist({ selected, onSelect, onAdd }: Props) {
  const { symbols, load, remove } = useWatchlistStore();
  const quotes = useQuotesStore((s) => s.bySymbol);

  useEffect(() => { load(); }, [load]);

  // Periodic tick so the stale indicator updates without a new quote arriving.
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setTick((x) => x + 1), 5000);
    return () => clearInterval(t);
  }, []);
  // Reference `tick` so React/ESLint sees the read; the value itself isn't used.
  void tick;

  return (
    <aside className="w-64 border-r border-slate-300/40 dark:border-white/10 flex flex-col">
      <div className="p-3 flex justify-between items-center border-b border-slate-300/40 dark:border-white/10">
        <span className="text-xs uppercase text-slate-500 dark:text-slate-400">Watchlist</span>
        <button onClick={onAdd} className="btn-secondary text-xs px-2 py-1">+ Add</button>
      </div>
      {symbols.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center text-center text-xs text-slate-500 dark:text-slate-400 px-4 py-8 gap-2">
          <p>관심 있는 종목을 추가해 보세요</p>
          <button
            onClick={onAdd}
            className="btn-primary"
          >
            + 종목 추가
          </button>
        </div>
      ) : (
      <ul className="flex-1 overflow-y-auto">
        {symbols.map((s) => {
          const q = quotes[quoteKey(s)];
          const isSelected = selected && s.kind === selected.kind && s.ticker === selected.ticker;
          const changePct = q?.change_24h ? Number(q.change_24h) * 100 : null;
          return (
            <li key={quoteKey(s)}
                onClick={() => onSelect(s)}
                className={clsx(
                  "px-3 py-2 cursor-pointer flex justify-between items-center",
                  isSelected
                    ? "bg-emerald-600/15 dark:bg-emerald-500/15"
                    : "hover:bg-white/30 dark:hover:bg-white/5",
                )}>
              <div>
                <div className="text-sm text-slate-700 dark:text-slate-200">{q?.display_name ?? s.ticker}</div>
                <div className="text-[10px] text-slate-500 dark:text-slate-500 uppercase">
                  {q?.display_name ? `${s.ticker} · ${s.kind}` : s.kind}
                </div>
              </div>
              <div className="flex items-center gap-2">
                {q && isStale(q.observed_at) && (
                  <span
                    title="Stale (>30s)"
                    aria-label="stale"
                    className="w-1.5 h-1.5 rounded-full bg-slate-400 dark:bg-slate-500"
                  />
                )}
                <div className="text-right">
                  <div className="text-sm tabular-nums text-slate-700 dark:text-slate-200">{q ? formatPrice(q.price) : "—"}</div>
                  <div className={clsx("text-[10px] tabular-nums",
                    changePct === null
                      ? "text-slate-500 dark:text-slate-500"
                      : changePct >= 0
                        ? "text-emerald-600 dark:text-emerald-400"
                        : "text-rose-600 dark:text-rose-400")}>
                    {changePct === null ? "" : `${changePct >= 0 ? "+" : ""}${changePct.toFixed(2)}%`}
                  </div>
                </div>
                <button onClick={(e) => { e.stopPropagation(); remove(s); }}
                  className="ml-2 text-slate-500 dark:text-slate-600 hover:text-rose-600 dark:hover:text-rose-400 text-xs">×</button>
              </div>
            </li>
          );
        })}
      </ul>
      )}
    </aside>
  );
}

function isStale(observed_at: string): boolean {
  const t = Date.parse(observed_at);
  if (Number.isNaN(t)) return false;
  return Date.now() - t > 30_000;
}

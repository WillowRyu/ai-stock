import { useEffect } from "react";
import clsx from "clsx";
import { useWatchlistStore } from "../lib/state/watchlistStore";
import { useQuotesStore, quoteKey } from "../lib/state/quotesStore";
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

  return (
    <aside className="w-64 border-r border-slate-800 flex flex-col">
      <div className="p-3 flex justify-between items-center border-b border-slate-800">
        <span className="text-xs uppercase text-slate-400">Watchlist</span>
        <button onClick={onAdd} className="text-xs px-2 py-1 rounded bg-slate-800 hover:bg-slate-700">+ Add</button>
      </div>
      <ul className="flex-1 overflow-y-auto">
        {symbols.map((s) => {
          const q = quotes[quoteKey(s)];
          const isSelected = selected && s.kind === selected.kind && s.ticker === selected.ticker;
          const changePct = q?.change_24h ? Number(q.change_24h) * 100 : null;
          return (
            <li key={quoteKey(s)}
                onClick={() => onSelect(s)}
                className={clsx("px-3 py-2 cursor-pointer flex justify-between items-center", isSelected && "bg-slate-800")}>
              <div>
                <div className="text-sm">{s.ticker}</div>
                <div className="text-[10px] text-slate-500 uppercase">{s.kind}</div>
              </div>
              <div className="text-right">
                <div className="text-sm tabular-nums">{q?.price ?? "—"}</div>
                <div className={clsx("text-[10px] tabular-nums",
                  changePct === null ? "text-slate-500" : changePct >= 0 ? "text-emerald-400" : "text-rose-400")}>
                  {changePct === null ? "" : `${changePct >= 0 ? "+" : ""}${changePct.toFixed(2)}%`}
                </div>
              </div>
              <button onClick={(e) => { e.stopPropagation(); remove(s); }}
                className="ml-2 text-slate-600 hover:text-rose-400 text-xs">×</button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}

import clsx from "clsx";
import { formatPrice } from "../../lib/format";
import type { QuoteDto } from "../../lib/ipc";

export function WidgetRow({ q }: { q: QuoteDto }) {
  const change = q.change_24h ? Number(q.change_24h) * 100 : null;
  return (
    <div className="flex justify-between items-center text-xs px-2 py-1">
      <span className="opacity-90 truncate max-w-[6rem]" title={q.symbol.ticker}>
        {q.display_name ?? q.symbol.ticker}
      </span>
      <span className="tabular-nums">{formatPrice(q.price)}</span>
      <span className={clsx("tabular-nums w-14 text-right",
        change === null ? "text-slate-400" : change >= 0 ? "text-emerald-400" : "text-rose-400")}>
        {change === null ? "" : `${change >= 0 ? "+" : ""}${change.toFixed(2)}%`}
      </span>
    </div>
  );
}

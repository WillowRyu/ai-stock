import clsx from "clsx";
import { formatPrice } from "../../lib/format";
import type { QuoteDto } from "../../lib/ipc";

export function WidgetRow({ q }: { q: QuoteDto }) {
  const change = q.change_24h ? Number(q.change_24h) * 100 : null;
  return (
    <div className="flex justify-between items-center text-xs px-2 py-1">
      <span className="truncate max-w-[6rem] text-slate-700 dark:text-slate-300" title={q.symbol.ticker}>
        {q.display_name ?? q.symbol.ticker}
      </span>
      <span className="tabular-nums text-slate-900 dark:text-slate-100">{formatPrice(q.price)}</span>
      <span className={clsx("tabular-nums w-14 text-right",
        change === null
          ? "text-slate-500 dark:text-slate-500"
          : change >= 0
            ? "text-emerald-600 dark:text-emerald-400"
            : "text-rose-600 dark:text-rose-400")}>
        {change === null ? "" : `${change >= 0 ? "+" : ""}${change.toFixed(2)}%`}
      </span>
    </div>
  );
}

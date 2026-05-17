import clsx from "clsx";
import { formatPrice } from "../../lib/format";
import type { QuoteDto } from "../../lib/ipc";

export function WidgetRow({ q }: { q: QuoteDto }) {
  const change = q.change_24h ? Number(q.change_24h) * 100 : null;
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_5rem_3.5rem] items-baseline gap-2 text-xs py-1">
      <span
        className="truncate text-slate-700 dark:text-slate-300"
        title={q.symbol.ticker}
      >
        {q.display_name ?? q.symbol.ticker}
      </span>
      <span className="text-right tabular-nums text-slate-900 dark:text-slate-100">
        {formatPrice(q.price)}
      </span>
      <span
        className={clsx(
          "text-right tabular-nums",
          change === null
            ? "text-slate-500 dark:text-slate-500"
            : change >= 0
              ? "text-emerald-600 dark:text-emerald-400"
              : "text-rose-600 dark:text-rose-400",
        )}
      >
        {change === null ? "" : `${change >= 0 ? "+" : ""}${change.toFixed(2)}%`}
      </span>
    </div>
  );
}

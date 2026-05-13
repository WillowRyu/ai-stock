import clsx from "clsx";
import { useToastStore } from "../lib/state/toastStore";

export function Toasts() {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);
  return (
    <div className="fixed bottom-4 right-4 flex flex-col gap-2 z-50 pointer-events-none">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={clsx(
            "pointer-events-auto backdrop-blur-md border rounded-md px-3 py-2 text-xs shadow-lg max-w-xs",
            t.kind === "error" && "bg-rose-100/80 dark:bg-rose-950/70 border-rose-300/50 dark:border-rose-800 text-rose-900 dark:text-rose-100",
            t.kind === "warning" && "bg-amber-100/80 dark:bg-amber-950/70 border-amber-300/50 dark:border-amber-800 text-amber-900 dark:text-amber-100",
            t.kind === "info" && "bg-white/70 dark:bg-slate-900/70 border-slate-300/50 dark:border-slate-700 text-slate-900 dark:text-slate-100",
          )}
        >
          <div className="flex justify-between items-start gap-2">
            <div>
              <div className="font-semibold">{t.title}</div>
              {t.body && <div className="opacity-80">{t.body}</div>}
            </div>
            <button
              onClick={() => dismiss(t.id)}
              className="opacity-60 hover:opacity-100"
              aria-label="dismiss"
            >
              ×
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}

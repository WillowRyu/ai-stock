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
            "pointer-events-auto border rounded-md px-3 py-2 text-xs shadow-lg max-w-xs",
            t.kind === "error" && "bg-rose-950 border-rose-700 text-rose-100",
            t.kind === "warning" && "bg-amber-950 border-amber-700 text-amber-100",
            t.kind === "info" && "bg-slate-900 border-slate-700 text-slate-100",
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

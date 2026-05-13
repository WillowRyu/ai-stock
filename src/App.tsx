import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Watchlist } from "./components/Watchlist";
import { DetailPane } from "./components/DetailPane";
import { AddSymbolDialog } from "./components/AddSymbolDialog";
import { PortfolioPanel } from "./components/PortfolioPanel";
import { AlertsPanel } from "./components/AlertsPanel";
import { AiPanel } from "./components/AiPanel";
import { Toasts } from "./components/Toasts";
import { useQuotesStore } from "./lib/state/quotesStore";
import { onQuoteUpdate, onProviderError, ipc, type SymbolDto } from "./lib/ipc";
import { usePortfolioStore } from "./lib/state/portfolioStore";
import { useToastStore } from "./lib/state/toastStore";

export default function App() {
  const [selected, setSelected] = useState<SymbolDto | null>(null);
  const [adding, setAdding] = useState(false);
  const [showAlerts, setShowAlerts] = useState(false);
  const [showAi, setShowAi] = useState(false);
  const apply = useQuotesStore((s) => s.apply);
  const refreshPortfolio = usePortfolioStore((s) => s.refresh);
  const pushToast = useToastStore((s) => s.push);

  useEffect(() => {
    ipc.quotesSnapshot().then(apply);
    refreshPortfolio();
    // Per-(symbol, provider) toast dedupe window: rate-limit repeated errors
    // (e.g. one outage producing one error per poll per symbol).
    const lastShown = new Map<string, number>();
    const TOAST_DEDUPE_MS = 60_000;
    const subs: Array<Promise<() => void>> = [
      onQuoteUpdate((updates) => {
        apply(updates);
        refreshPortfolio();
      }),
      onProviderError((err) => {
        const key = `${err.symbol_canonical}|${err.provider}`;
        const now = Date.now();
        const last = lastShown.get(key) ?? 0;
        if (now - last < TOAST_DEDUPE_MS) return;
        lastShown.set(key, now);
        pushToast({
          kind: "warning",
          title: `데이터 소스 오류 — ${err.symbol_canonical}`,
          body: err.provider ? `${err.provider}: ${err.error}` : err.error,
          ttl_ms: 6000,
        });
      }),
    ];
    return () => {
      subs.forEach((p) => p.then((fn) => fn()));
    };
  }, [apply, refreshPortfolio, pushToast]);

  return (
    <div className="h-screen flex flex-col">
      <header className="h-10 border-b border-slate-300/40 dark:border-white/10 px-4 flex items-center text-sm">
        <span className="font-semibold text-slate-900 dark:text-slate-100">ai-stock</span>
        <div className="ml-auto flex gap-2">
          <button onClick={() => setShowAi(true)} className="btn-secondary text-xs px-2 py-1">AI</button>
          <button onClick={() => setShowAlerts(true)} className="btn-secondary text-xs px-2 py-1">알림</button>
          <button onClick={() => invoke("widget_toggle")} className="btn-secondary text-xs px-2 py-1">위젯</button>
        </div>
      </header>
      <div className="flex flex-1 min-h-0">
        <Watchlist selected={selected} onSelect={setSelected} onAdd={() => setAdding(true)} />
        <DetailPane symbol={selected} />
        <PortfolioPanel />
      </div>
      {adding && <AddSymbolDialog onClose={() => setAdding(false)} />}
      {showAlerts && <AlertsPanel onClose={() => setShowAlerts(false)} />}
      {showAi && <AiPanel symbol={selected} onClose={() => setShowAi(false)} />}
      <Toasts />
    </div>
  );
}

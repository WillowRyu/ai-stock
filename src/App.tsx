import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Watchlist } from "./components/Watchlist";
import { DetailPane } from "./components/DetailPane";
import { AddSymbolDialog } from "./components/AddSymbolDialog";
import { PortfolioPanel } from "./components/PortfolioPanel";
import { AlertsPanel } from "./components/AlertsPanel";
import { AiPanel } from "./components/AiPanel";
import { useQuotesStore } from "./lib/state/quotesStore";
import { onQuoteUpdate, ipc, type SymbolDto } from "./lib/ipc";
import { usePortfolioStore } from "./lib/state/portfolioStore";

export default function App() {
  const [selected, setSelected] = useState<SymbolDto | null>(null);
  const [adding, setAdding] = useState(false);
  const [showAlerts, setShowAlerts] = useState(false);
  const [showAi, setShowAi] = useState(false);
  const apply = useQuotesStore((s) => s.apply);
  const refreshPortfolio = usePortfolioStore((s) => s.refresh);

  useEffect(() => {
    ipc.quotesSnapshot().then(apply);
    refreshPortfolio();
    const unsub = onQuoteUpdate((updates) => {
      apply(updates);
      refreshPortfolio();
    });
    return () => { unsub.then((fn) => fn()); };
  }, [apply, refreshPortfolio]);

  return (
    <div className="h-screen flex flex-col">
      <header className="h-10 border-b border-slate-800 px-4 flex items-center text-sm">
        <span className="font-semibold">ai-stock</span>
        <div className="ml-auto flex gap-2">
          <button onClick={() => setShowAi(true)} className="text-xs px-2 py-1 rounded bg-slate-800">AI</button>
          <button onClick={() => setShowAlerts(true)} className="text-xs px-2 py-1 rounded bg-slate-800">알림</button>
          <button onClick={() => invoke("widget_toggle")} className="text-xs px-2 py-1 rounded bg-slate-800">위젯</button>
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
    </div>
  );
}

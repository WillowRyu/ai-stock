import React, { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { onQuoteUpdate, ipc } from "./lib/ipc";
import { useQuotesStore } from "./lib/state/quotesStore";
import { useSettingsStore } from "./lib/state/settingsStore";
import { WidgetRow } from "./components/widget/WidgetRow";
import "./index.css";

function Widget() {
  const quotes = useQuotesStore((s) => Object.values(s.bySymbol));
  const apply = useQuotesStore((s) => s.apply);
  const { settings, load, save } = useSettingsStore();
  const [opacity, setOpacity] = useState(0.85);

  useEffect(() => {
    ipc.quotesSnapshot().then(apply);
    const unsub = onQuoteUpdate(apply);
    load().then(() => {
      const s = useSettingsStore.getState().settings;
      if (s) setOpacity(s.widget_opacity);
    });
    return () => { unsub.then((fn) => fn()); };
  }, [apply, load]);

  async function changeOpacity(v: number) {
    setOpacity(v);
    if (settings) await save({ ...settings, widget_opacity: v });
  }

  return (
    <div
      className="rounded-lg select-none flex flex-col"
      style={{ backgroundColor: `rgba(15,23,42,${opacity})`, color: "#e2e8f0", height: "100vh" }}
    >
      <div
        className="flex justify-between items-center text-[10px] text-slate-400 px-2 pt-2 pb-1"
        data-tauri-drag-region
      >
        <span data-tauri-drag-region>ai-stock</span>
        <input
          type="range" min={0.1} max={1} step={0.05}
          value={opacity}
          onChange={(e) => changeOpacity(Number(e.target.value))}
          onMouseDown={(e) => e.stopPropagation()}
          className="w-20"
        />
        <button
          onClick={() => getCurrentWebviewWindow().hide()}
          className="hover:text-slate-200 px-1"
        >×</button>
      </div>
      <div className="flex-1 px-2 pb-2 overflow-y-auto">
        {quotes.slice(0, 5).map((q) => (
          <WidgetRow
            key={q.symbol.quote_currency ? `${q.symbol.kind}:${q.symbol.ticker}:${q.symbol.quote_currency}` : `${q.symbol.kind}:${q.symbol.ticker}`}
            q={q}
          />
        ))}
        {quotes.length === 0 && <div className="text-center text-[11px] text-slate-500 py-2">no quotes yet</div>}
      </div>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode><Widget /></React.StrictMode>,
);

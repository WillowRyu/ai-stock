import { useState } from "react";
import { useWatchlistStore } from "../lib/state/watchlistStore";
import type { AssetKind, SymbolDto } from "../lib/ipc";

export function AddSymbolDialog({ onClose }: { onClose(): void }) {
  const add = useWatchlistStore((s) => s.add);
  const [kind, setKind] = useState<AssetKind>("crypto");
  const [ticker, setTicker] = useState("BTC");
  const [quote, setQuote] = useState("USD");
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const symbol: SymbolDto = {
      kind,
      ticker: ticker.toUpperCase(),
      quote_currency: kind === "crypto" ? quote.toUpperCase() : null,
    };
    try { await add(symbol); onClose(); } catch (err) { setError(String(err)); }
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <form onClick={(e) => e.stopPropagation()} onSubmit={submit}
            className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-80 space-y-3">
        <h3 className="text-lg font-semibold">종목 추가</h3>
        <label className="block text-xs">자산 유형
          <select value={kind} onChange={(e) => setKind(e.target.value as AssetKind)}
                  className="mt-1 w-full bg-slate-800 rounded p-1.5">
            <option value="crypto">Crypto</option>
            <option value="us">US Equity</option>
            <option value="kr">KR Equity (M2)</option>
            <option value="fx">Forex</option>
            <option value="com">Commodity</option>
          </select>
        </label>
        <label className="block text-xs">티커
          <input value={ticker} onChange={(e) => setTicker(e.target.value)}
                 className="mt-1 w-full bg-slate-800 rounded p-1.5" />
        </label>
        {kind === "crypto" && (
          <label className="block text-xs">호가 통화
            <input value={quote} onChange={(e) => setQuote(e.target.value)}
                   className="mt-1 w-full bg-slate-800 rounded p-1.5" />
          </label>
        )}
        {error && <div className="text-rose-400 text-xs">{error}</div>}
        <div className="flex gap-2 justify-end">
          <button type="button" onClick={onClose} className="px-3 py-1 text-sm rounded bg-slate-800">취소</button>
          <button type="submit" className="px-3 py-1 text-sm rounded bg-emerald-600">추가</button>
        </div>
      </form>
    </div>
  );
}

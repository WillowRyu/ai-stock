import { useState } from "react";
import { useWatchlistStore } from "../lib/state/watchlistStore";
import { Select } from "./Select";
import type { AssetKind, SymbolDto } from "../lib/ipc";

const KIND_OPTIONS = [
  { value: "crypto", label: "Crypto" },
  { value: "us", label: "US Equity" },
  { value: "kr", label: "KR Equity" },
  { value: "fx", label: "Forex" },
  { value: "com", label: "Commodity" },
];

interface Preset {
  ticker: string;
  label: string;
  quote_currency?: string;
}

const PRESETS: Record<AssetKind, Preset[]> = {
  crypto: [
    { ticker: "BTC", label: "BTC · Bitcoin", quote_currency: "USDT" },
    { ticker: "ETH", label: "ETH · Ethereum", quote_currency: "USDT" },
    { ticker: "SOL", label: "SOL · Solana", quote_currency: "USDT" },
    { ticker: "XRP", label: "XRP · Ripple", quote_currency: "USDT" },
    { ticker: "BNB", label: "BNB · Binance Coin", quote_currency: "USDT" },
    { ticker: "DOGE", label: "DOGE · Dogecoin", quote_currency: "USDT" },
    { ticker: "ADA", label: "ADA · Cardano", quote_currency: "USDT" },
  ],
  us: [
    { ticker: "AAPL", label: "AAPL · Apple" },
    { ticker: "NVDA", label: "NVDA · NVIDIA" },
    { ticker: "MSFT", label: "MSFT · Microsoft" },
    { ticker: "GOOGL", label: "GOOGL · Alphabet" },
    { ticker: "AMZN", label: "AMZN · Amazon" },
    { ticker: "TSLA", label: "TSLA · Tesla" },
    { ticker: "META", label: "META · Meta" },
  ],
  kr: [
    { ticker: "005930", label: "005930 · 삼성전자" },
    { ticker: "000660", label: "000660 · SK하이닉스" },
    { ticker: "035420", label: "035420 · NAVER" },
    { ticker: "035720", label: "035720 · 카카오" },
    { ticker: "207940", label: "207940 · 삼성바이오로직스" },
    { ticker: "005380", label: "005380 · 현대차" },
    { ticker: "051910", label: "051910 · LG화학" },
  ],
  fx: [
    { ticker: "USDKRW=X", label: "USD/KRW" },
    { ticker: "EURUSD=X", label: "EUR/USD" },
    { ticker: "JPYUSD=X", label: "JPY/USD" },
    { ticker: "GBPUSD=X", label: "GBP/USD" },
  ],
  com: [
    { ticker: "GC=F", label: "Gold" },
    { ticker: "CL=F", label: "Crude Oil" },
    { ticker: "SI=F", label: "Silver" },
  ],
};

export function AddSymbolDialog({ onClose }: { onClose(): void }) {
  const add = useWatchlistStore((s) => s.add);
  const [kind, setKind] = useState<AssetKind>("crypto");
  const [ticker, setTicker] = useState("BTC");
  const [quote, setQuote] = useState("USDT");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function addSymbol(s: SymbolDto) {
    setError(null);
    setBusy(true);
    try {
      await add(s);
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    await addSymbol({
      kind,
      ticker: ticker.toUpperCase(),
      quote_currency: kind === "crypto" ? quote.toUpperCase() : null,
    });
  }

  async function pickPreset(p: Preset) {
    await addSymbol({
      kind,
      ticker: p.ticker.toUpperCase(),
      quote_currency: kind === "crypto" ? (p.quote_currency ?? "USDT").toUpperCase() : null,
    });
  }

  const presets = PRESETS[kind] ?? [];

  return (
    <div className="fixed inset-0 z-50 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <form
        onClick={(e) => e.stopPropagation()}
        onSubmit={submit}
        className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-[28rem] space-y-3"
      >
        <h3 className="text-lg font-semibold">종목 추가</h3>

        <div className="block text-sm">
          <span>자산 유형</span>
          <Select
            value={kind}
            options={KIND_OPTIONS}
            onChange={(v) => {
              const k = v as AssetKind;
              setKind(k);
              const first = PRESETS[k]?.[0];
              if (first) {
                setTicker(first.ticker);
                if (k === "crypto") setQuote(first.quote_currency ?? "USDT");
              }
            }}
            className="mt-1"
          />
        </div>

        {presets.length > 0 && (
          <div>
            <div className="text-[10px] uppercase text-slate-500 mb-1">자주 쓰는 종목 (클릭하면 즉시 추가)</div>
            <div className="flex flex-wrap gap-1">
              {presets.map((p) => (
                <button
                  type="button"
                  key={p.ticker}
                  disabled={busy}
                  onClick={() => pickPreset(p)}
                  className="text-[11px] bg-slate-800 hover:bg-slate-700 rounded px-2 py-1 disabled:opacity-50"
                >
                  {p.label}
                </button>
              ))}
            </div>
          </div>
        )}

        <div className="border-t border-slate-800 pt-3">
          <div className="text-[10px] uppercase text-slate-500 mb-1">직접 입력</div>
          <label className="block text-sm">
            티커
            <input
              value={ticker}
              onChange={(e) => setTicker(e.target.value)}
              className="mt-1 w-full bg-slate-800 rounded px-3 py-2.5 text-base"
            />
          </label>
          {kind === "crypto" && (
            <label className="block text-sm mt-2">
              호가 통화 (예: USDT, USD)
              <input
                value={quote}
                onChange={(e) => setQuote(e.target.value)}
                className="mt-1 w-full bg-slate-800 rounded px-3 py-2.5 text-base"
              />
            </label>
          )}
        </div>

        {error && <div className="text-rose-400 text-xs">{error}</div>}

        <div className="flex gap-2 justify-end">
          <button type="button" onClick={onClose} className="px-3 py-1 text-sm rounded bg-slate-800">취소</button>
          <button type="submit" disabled={busy} className="px-3 py-1 text-sm rounded bg-emerald-600 disabled:opacity-50">
            {busy ? "추가 중..." : "추가"}
          </button>
        </div>
      </form>
    </div>
  );
}

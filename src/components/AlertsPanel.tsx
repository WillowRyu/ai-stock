import { useEffect, useState } from "react";
import { alertsIpc, type AlertRuleDto, type AssetKind } from "../lib/ipc";

export function AlertsPanel({ onClose }: { onClose(): void }) {
  const [rules, setRules] = useState<AlertRuleDto[]>([]);
  const [draft, setDraft] = useState({
    kind: "crypto" as AssetKind,
    ticker: "BTC",
    quote: "USD",
    condition: "above" as "above" | "below",
    amount: "70000",
    ccy: "USD",
  });

  async function load() {
    setRules(await alertsIpc.list());
  }
  useEffect(() => {
    load();
  }, []);

  async function create(e: React.FormEvent) {
    e.preventDefault();
    await alertsIpc.create({
      id: 0,
      symbol: {
        kind: draft.kind,
        ticker: draft.ticker.toUpperCase(),
        quote_currency: draft.kind === "crypto" ? draft.quote.toUpperCase() : null,
      },
      condition: draft.condition,
      threshold_amount: draft.amount,
      threshold_currency: draft.ccy.toUpperCase(),
      enabled: true,
      cooldown_secs: 60,
    });
    await load();
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-[28rem] space-y-3"
      >
        <div className="flex justify-between">
          <h3 className="text-lg font-semibold">알림</h3>
          <button onClick={onClose}>×</button>
        </div>

        <form onSubmit={create} className="grid grid-cols-2 gap-2 text-xs">
          <select
            value={draft.kind}
            onChange={(e) => setDraft({ ...draft, kind: e.target.value as AssetKind })}
            className="bg-slate-800 rounded p-1.5"
          >
            <option value="crypto">Crypto</option>
            <option value="us">US Equity</option>
          </select>
          <input
            value={draft.ticker}
            onChange={(e) => setDraft({ ...draft, ticker: e.target.value })}
            className="bg-slate-800 rounded p-1.5"
          />
          {draft.kind === "crypto" && (
            <input
              value={draft.quote}
              onChange={(e) => setDraft({ ...draft, quote: e.target.value })}
              placeholder="quote ccy"
              className="bg-slate-800 rounded p-1.5 col-span-2"
            />
          )}
          <select
            value={draft.condition}
            onChange={(e) => setDraft({ ...draft, condition: e.target.value as "above" | "below" })}
            className="bg-slate-800 rounded p-1.5"
          >
            <option value="above">상승</option>
            <option value="below">하락</option>
          </select>
          <input
            value={draft.amount}
            onChange={(e) => setDraft({ ...draft, amount: e.target.value })}
            placeholder="임계값"
            className="bg-slate-800 rounded p-1.5"
          />
          <input
            value={draft.ccy}
            onChange={(e) => setDraft({ ...draft, ccy: e.target.value })}
            placeholder="통화"
            className="bg-slate-800 rounded p-1.5"
          />
          <button type="submit" className="col-span-2 bg-emerald-600 rounded py-1.5">
            추가
          </button>
        </form>

        <ul className="text-xs space-y-1 max-h-60 overflow-y-auto">
          {rules.map((r) => (
            <li key={r.id} className="flex justify-between border-b border-slate-800 py-1">
              <span>
                {r.symbol.ticker} {r.condition === "above" ? "≥" : "≤"} {r.threshold_amount}{" "}
                {r.threshold_currency}
              </span>
              <button
                onClick={async () => {
                  await alertsIpc.delete(r.id);
                  await load();
                }}
                className="text-rose-400"
              >
                삭제
              </button>
            </li>
          ))}
          {rules.length === 0 && (
            <li className="text-slate-500 text-center py-2">규칙 없음</li>
          )}
        </ul>
      </div>
    </div>
  );
}

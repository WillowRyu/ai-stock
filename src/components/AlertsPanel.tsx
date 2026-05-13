import { useEffect, useState } from "react";
import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";
import { alertsIpc, type AlertConditionKind, type AlertRuleDto, type AssetKind } from "../lib/ipc";

async function ensureNotificationPermission(): Promise<boolean> {
  try {
    let granted = await isPermissionGranted();
    if (!granted) {
      const result = await requestPermission();
      granted = result === "granted";
    }
    return granted;
  } catch {
    // Older API / unsupported — assume ok and let the backend handle it.
    return true;
  }
}

const CONDITION_OPTIONS: { value: AlertConditionKind; label: string; needsThreshold: boolean; needsCurrency: boolean }[] = [
  { value: "above", label: "가격 ≥ 임계값", needsThreshold: true, needsCurrency: true },
  { value: "below", label: "가격 ≤ 임계값", needsThreshold: true, needsCurrency: true },
  { value: "rsi_above", label: "RSI(14) ≥ 임계값 (과매수)", needsThreshold: true, needsCurrency: false },
  { value: "rsi_below", label: "RSI(14) ≤ 임계값 (과매도)", needsThreshold: true, needsCurrency: false },
  { value: "macd_golden", label: "MACD 골든크로스", needsThreshold: false, needsCurrency: false },
  { value: "macd_death", label: "MACD 데드크로스", needsThreshold: false, needsCurrency: false },
];

function describeRule(r: AlertRuleDto): string {
  const t = r.symbol.ticker;
  const amount = r.threshold_amount;
  const ccy = r.threshold_currency;
  switch (r.condition) {
    case "above":     return `${t} ≥ ${amount} ${ccy ?? ""}`.trim();
    case "below":     return `${t} ≤ ${amount} ${ccy ?? ""}`.trim();
    case "rsi_above": return `${t} RSI(14) ≥ ${amount}`;
    case "rsi_below": return `${t} RSI(14) ≤ ${amount}`;
    case "macd_golden": return `${t} MACD 골든크로스`;
    case "macd_death":  return `${t} MACD 데드크로스`;
  }
}

export function AlertsPanel({ onClose }: { onClose(): void }) {
  const [rules, setRules] = useState<AlertRuleDto[]>([]);
  const [draft, setDraft] = useState({
    kind: "crypto" as AssetKind,
    ticker: "BTC",
    quote: "USDT",
    condition: "above" as AlertConditionKind,
    amount: "70000",
    ccy: "USD",
  });
  const [error, setError] = useState<string | null>(null);

  const cond = CONDITION_OPTIONS.find((c) => c.value === draft.condition)!;

  async function load() {
    try { setRules(await alertsIpc.list()); } catch (e) { setError(String(e)); }
  }
  useEffect(() => { load(); }, []);

  async function create(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const ok = await ensureNotificationPermission();
    if (!ok) {
      setError("알림 권한이 거부되어 알림이 표시되지 않습니다. 시스템 설정 → 알림에서 허용해 주세요.");
      // Continue creating the rule anyway — it'll fire silently if the user changes their mind.
    }
    try {
      await alertsIpc.create({
        id: 0,
        symbol: {
          kind: draft.kind,
          ticker: draft.ticker.toUpperCase(),
          quote_currency: draft.kind === "crypto" ? draft.quote.toUpperCase() : null,
        },
        condition: draft.condition,
        threshold_amount: cond.needsThreshold ? draft.amount : null,
        threshold_currency: cond.needsCurrency ? draft.ccy.toUpperCase() : null,
        enabled: true,
        cooldown_secs: 60,
      });
      await load();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="glass-panel rounded-lg p-5 w-[32rem] space-y-3">
        <div className="flex justify-between">
          <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">알림</h3>
          <button onClick={onClose} className="text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-200">×</button>
        </div>

        <form onSubmit={create} className="grid grid-cols-2 gap-2 text-xs">
          <select value={draft.kind} onChange={(e) => setDraft({ ...draft, kind: e.target.value as AssetKind })} className="glass-inset rounded p-1.5 text-slate-700 dark:text-slate-200">
            <option value="crypto">Crypto</option>
            <option value="us">US Equity</option>
            <option value="kr">KR Equity</option>
          </select>
          <input value={draft.ticker} onChange={(e) => setDraft({ ...draft, ticker: e.target.value })} placeholder="ticker" className="glass-inset rounded p-1.5 text-slate-900 dark:text-slate-100" />
          {draft.kind === "crypto" && (
            <input value={draft.quote} onChange={(e) => setDraft({ ...draft, quote: e.target.value })} placeholder="quote currency" className="glass-inset rounded p-1.5 col-span-2 text-slate-900 dark:text-slate-100" />
          )}

          <select
            value={draft.condition}
            onChange={(e) => setDraft({ ...draft, condition: e.target.value as AlertConditionKind })}
            className="glass-inset rounded p-1.5 col-span-2 text-slate-700 dark:text-slate-200"
          >
            {CONDITION_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>{o.label}</option>
            ))}
          </select>

          {cond.needsThreshold && (
            <input
              value={draft.amount}
              onChange={(e) => setDraft({ ...draft, amount: e.target.value })}
              placeholder={cond.value.startsWith("rsi_") ? "RSI 임계값 (예: 70)" : "임계값"}
              className={"glass-inset rounded p-1.5 text-slate-900 dark:text-slate-100 " + (cond.needsCurrency ? "" : "col-span-2")}
            />
          )}
          {cond.needsCurrency && (
            <input value={draft.ccy} onChange={(e) => setDraft({ ...draft, ccy: e.target.value })} placeholder="통화" className="glass-inset rounded p-1.5 text-slate-900 dark:text-slate-100" />
          )}

          <button type="submit" className="col-span-2 btn-primary">추가</button>
        </form>

        {error && <div className="text-rose-600 dark:text-rose-400 text-xs">{error}</div>}

        <ul className="text-xs space-y-1 max-h-60 overflow-y-auto">
          {rules.map((r) => (
            <li key={r.id} className="flex justify-between border-b border-slate-300/40 dark:border-white/10 py-1 text-slate-700 dark:text-slate-200">
              <span>{describeRule(r)}</span>
              <button onClick={async () => { await alertsIpc.delete(r.id); await load(); }} className="text-rose-600 dark:text-rose-400">삭제</button>
            </li>
          ))}
          {rules.length === 0 && <li className="text-slate-500 dark:text-slate-500 text-center py-2">규칙 없음</li>}
        </ul>
      </div>
    </div>
  );
}

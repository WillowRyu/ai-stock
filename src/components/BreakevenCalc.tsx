import { useEffect, useMemo, useState } from "react";
import { useWatchlistStore } from "../lib/state/watchlistStore";
import { useQuotesStore, quoteKey } from "../lib/state/quotesStore";
import { ipc, type BreakevenPlanDto, type SymbolDto } from "../lib/ipc";
import { formatMoney } from "../lib/format";
import { Select } from "./Select";

// Native currency of an asset (mirrors PortfolioPanel's private helper; kept
// local to avoid restructuring an unrelated module).
function defaultCostCurrency(s: SymbolDto): string {
  if (s.quote_currency) return s.quote_currency;
  switch (s.kind) {
    case "us": return "USD";
    case "kr": return "KRW";
    case "fx":
    case "com":
    default:
      return "USD";
  }
}

function symbolLabel(s: SymbolDto): string {
  return s.quote_currency ? `${s.ticker} / ${s.quote_currency}` : s.ticker;
}

function fmtPct(s: string): string {
  const n = Number(s);
  if (!Number.isFinite(n)) return s;
  return n.toLocaleString(undefined, { minimumFractionDigits: 1, maximumFractionDigits: 1 });
}

function fmtQty(s: string): string {
  const n = Number(s);
  if (!Number.isFinite(n)) return s;
  return n.toLocaleString(undefined, { maximumFractionDigits: 6 });
}

function fmtRate(s: string): string {
  const n = Number(s);
  if (!Number.isFinite(n)) return s;
  return n.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

const PRESET_TARGETS = ["5", "10", "15"];

export function BreakevenCalc({ onClose }: { onClose(): void }) {
  const watchlist = useWatchlistStore((s) => s.symbols);
  const loadWatchlist = useWatchlistStore((s) => s.load);
  const quotes = useQuotesStore((s) => s.bySymbol);

  useEffect(() => {
    if (watchlist.length === 0) loadWatchlist();
  }, [watchlist.length, loadWatchlist]);

  const [selectedKey, setSelectedKey] = useState<string>("");
  const [avgInput, setAvgInput] = useState("");
  const [qtyInput, setQtyInput] = useState("");
  const [manualPrice, setManualPrice] = useState("");
  const [customPct, setCustomPct] = useState("");
  const [baseMode, setBaseMode] = useState<"native" | "KRW">("native");
  const [plan, setPlan] = useState<BreakevenPlanDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  const selectedSymbol = useMemo<SymbolDto | undefined>(
    () => watchlist.find((s) => quoteKey(s) === selectedKey),
    [watchlist, selectedKey],
  );
  const liveQuote = selectedSymbol ? quotes[quoteKey(selectedSymbol)] : undefined;
  const nativeCcy = selectedSymbol ? defaultCostCurrency(selectedSymbol) : "USD";
  // When the asset is already KRW-denominated there is no base-currency choice.
  const baseChoiceAvailable = nativeCcy !== "KRW";
  const baseCcy = baseMode === "KRW" && baseChoiceAvailable ? "KRW" : nativeCcy;
  // A live quote drives the price field (in native ccy); otherwise the user types
  // it directly in the base currency.
  const effectivePrice = liveQuote?.price ?? manualPrice;
  const priceCcy = liveQuote ? nativeCcy : baseCcy;

  const targets = useMemo(() => {
    const list = [...PRESET_TARGETS];
    if (customPct && Number(customPct) > 0 && !list.includes(customPct)) list.push(customPct);
    return list;
  }, [customPct]);

  // Debounced recompute; also re-fires whenever the live price ticks.
  useEffect(() => {
    const a = avgInput.trim();
    const q = qtyInput.trim();
    const p = effectivePrice.trim();
    if (!a || !q || !p || Number(a) <= 0 || Number(q) <= 0 || Number(p) <= 0) {
      setPlan(null);
      setError(null);
      return;
    }
    const handle = setTimeout(() => {
      ipc
        .breakevenPlan({
          avg_cost_amount: a,
          quantity: q,
          current_price_amount: p,
          price_currency: priceCcy,
          base_currency: baseCcy,
          targets_pct: targets,
        })
        .then((result) => {
          setPlan(result);
          setError(null);
        })
        .catch((e) => {
          setError(String(e));
          setPlan(null);
        });
    }, 150);
    return () => clearTimeout(handle);
  }, [avgInput, qtyInput, effectivePrice, priceCcy, baseCcy, targets]);

  const showKrwEcho =
    baseCcy === "KRW" &&
    !!liveQuote &&
    !!plan &&
    !plan.rate_missing &&
    plan.current_price_base != null &&
    plan.fx_rate_used != null;

  return (
    <div className="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center" onClick={onClose}>
      <div
        onClick={(e) => e.stopPropagation()}
        className="glass-panel rounded-lg p-5 w-[34rem] max-h-[90vh] overflow-y-auto space-y-4"
      >
        <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">🧮 본전·물타기 계산기</h3>

        <div className="block text-sm">
          <span className="text-slate-700 dark:text-slate-300">종목 (선택)</span>
          <Select
            value={selectedKey}
            options={[
              { value: "", label: "직접 입력" },
              ...watchlist.map((s) => ({ value: quoteKey(s), label: symbolLabel(s) })),
            ]}
            onChange={setSelectedKey}
            className="mt-1"
          />
        </div>

        <div className="grid grid-cols-3 gap-3">
          <label className="block text-sm">
            <span className="text-slate-700 dark:text-slate-300">평단 ({baseCcy})</span>
            <input
              value={avgInput}
              onChange={(e) => setAvgInput(e.target.value)}
              inputMode="decimal"
              placeholder="평균 매입가"
              className="mt-1 w-full glass-inset rounded px-3 py-2.5 text-base text-slate-900 dark:text-slate-100"
            />
          </label>
          <label className="block text-sm">
            <span className="text-slate-700 dark:text-slate-300">보유수량</span>
            <input
              value={qtyInput}
              onChange={(e) => setQtyInput(e.target.value)}
              inputMode="decimal"
              placeholder="예: 0.5"
              className="mt-1 w-full glass-inset rounded px-3 py-2.5 text-base text-slate-900 dark:text-slate-100"
            />
          </label>
          <label className="block text-sm">
            <span className="text-slate-700 dark:text-slate-300">현재가 ({priceCcy})</span>
            <input
              value={effectivePrice}
              onChange={(e) => setManualPrice(e.target.value)}
              readOnly={!!liveQuote}
              inputMode="decimal"
              placeholder="현재가"
              className={
                "mt-1 w-full glass-inset rounded px-3 py-2.5 text-base text-slate-900 dark:text-slate-100 " +
                (liveQuote ? "opacity-70" : "")
              }
            />
          </label>
        </div>
        {liveQuote && (
          <div className="text-xs text-emerald-600 dark:text-emerald-400">● 실시간 현재가 사용 중</div>
        )}
        {showKrwEcho && plan && (
          <div className="text-xs text-slate-500 dark:text-slate-400">
            원화 환산 현재가 ≈ {formatMoney(plan.current_price_base ?? "")} KRW · 환율 1 {nativeCcy} = {fmtRate(plan.fx_rate_used ?? "")} KRW
          </div>
        )}

        <div className="flex items-center gap-2 text-sm">
          {baseChoiceAvailable && (
            <>
              <span className="text-slate-700 dark:text-slate-300">기준 통화</span>
              {(["native", "KRW"] as const).map((m) => (
                <button
                  key={m}
                  type="button"
                  onClick={() => setBaseMode(m)}
                  className={
                    "px-2 py-1 rounded text-xs " +
                    (baseMode === m
                      ? "bg-emerald-600/15 dark:bg-emerald-500/15 text-emerald-700 dark:text-emerald-400"
                      : "btn-secondary")
                  }
                >
                  {m === "native" ? `네이티브 (${nativeCcy})` : "원화 (KRW)"}
                </button>
              ))}
            </>
          )}
          <label className="ml-auto flex items-center gap-1">
            <span className="text-slate-500 dark:text-slate-400 text-xs">직접 %</span>
            <input
              value={customPct}
              onChange={(e) => setCustomPct(e.target.value)}
              inputMode="decimal"
              placeholder="예: 8"
              className="w-16 glass-inset rounded px-2 py-1 text-xs text-slate-900 dark:text-slate-100"
            />
          </label>
        </div>

        {error && <div className="text-rose-600 dark:text-rose-400 text-xs">{error}</div>}

        {plan?.rate_missing ? (
          <p className="text-sm text-amber-600 dark:text-amber-400">
            원화 환율을 아직 불러오지 못했습니다. 네이티브 통화로 보거나 잠시 후 다시 시도하세요.
          </p>
        ) : !plan ? (
          <p className="text-sm text-slate-500 dark:text-slate-400">
            평단·보유수량·현재가를 입력하면 본전까지의 상승률과 물타기 시나리오가 계산됩니다.
          </p>
        ) : (
          <>
            <div className="glass-inset rounded p-3">
              <div className="text-xs text-slate-500 dark:text-slate-400">본전까지</div>
              {plan.is_underwater && plan.breakeven_gap_pct ? (
                <div className="text-base text-slate-900 dark:text-slate-100">
                  {baseCcy === "KRW" ? "원화 기준 " : ""}현재가가{" "}
                  <span className="font-semibold text-rose-600 dark:text-rose-400">
                    +{fmtPct(plan.breakeven_gap_pct)}%
                  </span>{" "}
                  오르면 본전
                </div>
              ) : (
                <div className="text-base text-emerald-700 dark:text-emerald-400">
                  이미 본전 이상 (+{fmtPct(plan.current_return_pct ?? "0")}%)
                </div>
              )}
            </div>

            {!plan.is_underwater ? (
              <p className="text-sm text-slate-500 dark:text-slate-400">
                현재가가 평단 이상이라 추가 매수로 평단을 낮출 수 없습니다.
              </p>
            ) : (
              <div className="space-y-1">
                <div className="text-xs text-slate-500 dark:text-slate-400">
                  물타기 시나리오 (최대 −{fmtPct(plan.max_reduction_pct)}%까지 가능)
                </div>
                <table className="w-full text-xs">
                  <thead className="text-slate-500 dark:text-slate-400">
                    <tr className="text-left">
                      <th className="py-1">평단 낮춤</th>
                      <th>목표 평단</th>
                      <th>추가 매수</th>
                      <th>추가 투자금</th>
                      <th>새 본전까지</th>
                    </tr>
                  </thead>
                  <tbody>
                    {plan.rows.map((r, i) => (
                      <tr key={i} className="border-t border-slate-300/40 dark:border-white/10">
                        <td className="py-1 tabular-nums">−{fmtPct(r.target_pct)}%</td>
                        {r.feasible ? (
                          <>
                            <td className="tabular-nums">{formatMoney(r.target_avg)}</td>
                            <td className="tabular-nums">{fmtQty(r.add_quantity)}</td>
                            <td className="tabular-nums">
                              {formatMoney(r.add_invest)} {baseCcy}
                            </td>
                            <td className="tabular-nums text-rose-600 dark:text-rose-400">
                              +{fmtPct(r.new_breakeven_gap_pct)}%
                            </td>
                          </>
                        ) : (
                          <td colSpan={4} className="text-slate-400 dark:text-slate-500">
                            불가능 — 현재가가 더 낮아야 함
                          </td>
                        )}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </>
        )}

        <div className="flex justify-end">
          <button type="button" onClick={onClose} className="btn-secondary text-sm">닫기</button>
        </div>
      </div>
    </div>
  );
}

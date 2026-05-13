import { useEffect, useRef, useState } from "react";
import {
  aiIpc, onAiChunk, onAiDone, onAiError,
  type AiProviderKind, type SymbolDto,
} from "../lib/ipc";

export function AiPanel({ symbol, onClose }: { symbol: SymbolDto | null; onClose(): void }) {
  const [provider, setProvider] = useState<AiProviderKind>("openai");
  const [hasKey, setHasKey] = useState(false);
  const [text, setText] = useState("");
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const unsubs = useRef<Array<() => void>>([]);

  useEffect(() => {
    aiIpc.hasKey(provider).then(setHasKey);
  }, [provider]);

  useEffect(() => {
    let mounted = true;
    Promise.all([
      onAiChunk((t) => mounted && setText((prev) => prev + t)),
      onAiDone(() => mounted && setRunning(false)),
      onAiError((e) => { if (mounted) { setRunning(false); setError(e); } }),
    ]).then((arr) => { unsubs.current = arr; });
    return () => { mounted = false; unsubs.current.forEach((u) => u()); };
  }, []);

  async function run() {
    if (!symbol) return;
    setError(null); setText(""); setRunning(true);
    try { await aiIpc.commentary(provider, symbol); }
    catch (e) { setError(String(e)); setRunning(false); }
  }

  return (
    <div className="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="glass-panel rounded-lg p-5 w-[36rem] space-y-3">
        <div className="flex justify-between items-center">
          <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">AI 해석 {symbol && `· ${symbol.ticker}`}</h3>
          <button onClick={onClose} className="text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-200">×</button>
        </div>
        <div className="flex gap-2 text-xs items-center">
          <select value={provider} onChange={(e) => setProvider(e.target.value as AiProviderKind)} className="glass-inset rounded p-1.5 text-slate-700 dark:text-slate-200">
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
            <option value="gemini">Gemini</option>
          </select>
          <span className={hasKey ? "text-emerald-600 dark:text-emerald-400" : "text-slate-500 dark:text-slate-500"}>
            {hasKey ? "키 설정됨" : "키 없음 (설정에서 입력)"}
          </span>
          <button onClick={run} disabled={!symbol || !hasKey || running}
            className="ml-auto btn-primary disabled:opacity-50">
            {running ? "생성 중..." : "해석 요청"}
          </button>
        </div>
        <div className="glass-inset rounded p-3 min-h-[12rem] whitespace-pre-wrap text-sm text-slate-700 dark:text-slate-200">
          {text || (symbol ? "버튼을 눌러 시작" : "워치리스트에서 종목 선택")}
        </div>
        {error && <div className="text-rose-600 dark:text-rose-400 text-xs">{error}</div>}
      </div>
    </div>
  );
}

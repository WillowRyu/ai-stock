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
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-[36rem] space-y-3">
        <div className="flex justify-between items-center">
          <h3 className="text-lg font-semibold">AI 해석 {symbol && `· ${symbol.ticker}`}</h3>
          <button onClick={onClose}>×</button>
        </div>
        <div className="flex gap-2 text-xs items-center">
          <select value={provider} onChange={(e) => setProvider(e.target.value as AiProviderKind)} className="bg-slate-800 rounded p-1.5">
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
            <option value="gemini">Gemini</option>
          </select>
          <span className={hasKey ? "text-emerald-400" : "text-slate-500"}>
            {hasKey ? "키 설정됨" : "키 없음 (설정에서 입력)"}
          </span>
          <button onClick={run} disabled={!symbol || !hasKey || running}
            className="ml-auto bg-emerald-600 disabled:bg-slate-700 rounded px-3 py-1.5">
            {running ? "생성 중..." : "해석 요청"}
          </button>
        </div>
        <div className="bg-slate-950 border border-slate-800 rounded p-3 min-h-[12rem] whitespace-pre-wrap text-sm">
          {text || (symbol ? "버튼을 눌러 시작" : "워치리스트에서 종목 선택")}
        </div>
        {error && <div className="text-rose-400 text-xs">{error}</div>}
      </div>
    </div>
  );
}

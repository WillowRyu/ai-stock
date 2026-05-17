import { useEffect, useRef, useState } from "react";
import {
  aiIpc, onAiChunk, onAiDone, onAiError,
  type AiProviderKind, type AiPromptKind, type SymbolDto,
} from "../lib/ipc";
import { useAiStore } from "../lib/state/aiStore";
import { quoteKey } from "../lib/state/quotesStore";

const PRESETS: { kind: AiPromptKind; label: string }[] = [
  { kind: "commentary", label: "시장 해석" },
  { kind: "chart_analysis", label: "차트·지표 분석" },
  { kind: "news_summary", label: "뉴스 요약" },
];

// The backend builds the real preset user message (a data dump). The chat view
// shows a friendly label instead — assistant replies stay identical either way.
function presetUserLabel(kind: AiPromptKind): string {
  switch (kind) {
    case "commentary": return "시장 해석 요청";
    case "chart_analysis": return "차트·지표 분석 요청";
    case "news_summary": return "뉴스 요약 요청";
  }
}

export function AiPanel({ symbol, onClose }: { symbol: SymbolDto | null; onClose(): void }) {
  const [provider, setProvider] = useState<AiProviderKind>("openai");
  const [hasKey, setHasKey] = useState(false);
  const [input, setInput] = useState("");
  const [error, setError] = useState<string | null>(null);

  const {
    bySymbol, streaming, pushUser, startAssistant, appendChunk,
    finishStreaming, failStreaming,
  } = useAiStore();

  const symKey = symbol ? quoteKey(symbol) : null;
  const messages = symKey ? bySymbol[symKey] ?? [] : [];

  // The chunk listener is registered once; it reads the current symbol key
  // through a ref so streamed text always lands on the active conversation.
  const symKeyRef = useRef<string | null>(symKey);
  symKeyRef.current = symKey;

  useEffect(() => {
    aiIpc.hasKey(provider).then(setHasKey);
  }, [provider]);

  useEffect(() => {
    let mounted = true;
    let localUnsubs: Array<() => void> = [];
    Promise.all([
      onAiChunk((t) => {
        if (mounted && symKeyRef.current) appendChunk(symKeyRef.current, t);
      }),
      onAiDone(() => { if (mounted) finishStreaming(); }),
      onAiError((e) => {
        if (!mounted) return;
        if (symKeyRef.current) failStreaming(symKeyRef.current);
        else finishStreaming();
        setError(e);
      }),
    ]).then((arr) => {
      if (mounted) localUnsubs = arr;
      else arr.forEach((u) => u());
    });
    return () => { mounted = false; localUnsubs.forEach((u) => u()); };
  }, [appendChunk, finishStreaming, failStreaming]);

  // Auto-cancel an in-flight stream when the user switches symbols.
  useEffect(() => {
    return () => {
      if (useAiStore.getState().streaming) aiIpc.cancel();
    };
  }, [symKey]);

  function startPreset(kind: AiPromptKind) {
    if (!symbol || !symKey || streaming) return;
    setError(null);
    pushUser(symKey, presetUserLabel(kind));
    startAssistant(symKey);
    aiIpc.startTurn(provider, symbol, kind).catch((e) => {
      failStreaming(symKey);
      setError(String(e));
    });
  }

  function send() {
    if (!symbol || !symKey || streaming || !hasKey) return;
    const text = input.trim();
    if (!text) return;
    setError(null);
    pushUser(symKey, text);
    startAssistant(symKey);
    setInput("");
    aiIpc.sendMessage(provider, symbol, text).catch((e) => {
      failStreaming(symKey);
      setError(String(e));
    });
  }

  return (
    <div
      className="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center"
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="glass-panel rounded-lg p-5 w-[36rem] flex flex-col gap-3 max-h-[80vh]"
      >
        <div className="flex justify-between items-center">
          <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">
            AI 어시스턴트 {symbol && `· ${symbol.ticker}`}
          </h3>
          <button
            onClick={onClose}
            className="text-slate-500 dark:text-slate-400 hover:text-slate-700 dark:hover:text-slate-200"
          >
            ×
          </button>
        </div>

        <div className="flex gap-2 text-xs items-center">
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value as AiProviderKind)}
            className="glass-inset rounded p-1.5 text-slate-700 dark:text-slate-200"
          >
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
            <option value="gemini">Gemini</option>
          </select>
          <span
            className={
              hasKey
                ? "text-emerald-600 dark:text-emerald-400"
                : "text-slate-500 dark:text-slate-500"
            }
          >
            {hasKey ? "키 설정됨" : "키 없음 (설정에서 입력)"}
          </span>
        </div>

        <div className="flex gap-1.5 flex-wrap">
          {PRESETS.map((p) => (
            <button
              key={p.kind}
              onClick={() => startPreset(p.kind)}
              disabled={!symbol || !hasKey || streaming}
              className="glass-inset rounded px-2.5 py-1 text-xs text-slate-700 dark:text-slate-200 disabled:opacity-40 hover:bg-white/40 dark:hover:bg-white/10"
            >
              {p.label}
            </button>
          ))}
        </div>

        <div className="glass-inset rounded p-3 flex-1 overflow-y-auto min-h-[14rem] space-y-3 text-sm">
          {messages.length === 0 && (
            <div className="text-slate-500 dark:text-slate-500">
              {symbol
                ? "위 버튼으로 분석을 시작하거나 질문을 입력하세요."
                : "워치리스트에서 종목을 선택하세요."}
            </div>
          )}
          {messages.map((m, i) => (
            <div key={i} className={m.role === "user" ? "text-right" : "text-left"}>
              <div
                className={
                  "inline-block rounded-lg px-3 py-2 whitespace-pre-wrap " +
                  (m.role === "user"
                    ? "bg-sky-500/20 text-slate-800 dark:text-slate-100"
                    : "bg-white/40 dark:bg-white/10 text-slate-700 dark:text-slate-200")
                }
              >
                {m.content ||
                  (streaming && i === messages.length - 1 ? "생성 중..." : "")}
              </div>
            </div>
          ))}
        </div>

        {error && <div className="text-rose-600 dark:text-rose-400 text-xs">{error}</div>}

        <div className="flex gap-2">
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") send(); }}
            disabled={!symbol || !hasKey}
            placeholder="후속 질문 입력..."
            className="glass-inset rounded p-2 flex-1 text-sm text-slate-700 dark:text-slate-200 disabled:opacity-40"
          />
          {streaming ? (
            <button
              onClick={() => aiIpc.cancel()}
              className="btn-primary bg-rose-500 hover:bg-rose-600"
            >
              중지
            </button>
          ) : (
            <button
              onClick={send}
              disabled={!symbol || !hasKey || !input.trim()}
              className="btn-primary disabled:opacity-50"
            >
              전송
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

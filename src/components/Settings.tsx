import { useEffect, useState } from "react";
import { useSettingsStore } from "../lib/state/settingsStore";
import { aiIpc, type AiProviderKind, type AppSettingsDto } from "../lib/ipc";

export function Settings({ onClose }: { onClose(): void }) {
  const { load, save } = useSettingsStore();
  const [draft, setDraft] = useState<AppSettingsDto | null>(null);
  const [keyDraft, setKeyDraft] = useState<{ provider: AiProviderKind; key: string }>({ provider: "openai", key: "" });
  useEffect(() => { load().then(() => setDraft(useSettingsStore.getState().settings)); }, [load]);
  if (!draft) return null;

  async function commit() {
    if (!draft) return;
    await save(draft);
    onClose();
  }

  async function saveKey() {
    if (!keyDraft.key) return;
    await aiIpc.setKey(keyDraft.provider, keyDraft.key);
    setKeyDraft({ ...keyDraft, key: "" });
  }
  async function clearKey() {
    await aiIpc.clearKey(keyDraft.provider);
  }

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="bg-slate-900 border border-slate-700 rounded-lg p-5 w-96 space-y-3">
        <h3 className="text-lg font-semibold">설정</h3>
        <label className="block text-xs">폴링 주기 (초)
          <input type="number" min={1} max={300} value={draft.poll_interval_secs}
                 onChange={(e) => setDraft({ ...draft, poll_interval_secs: Number(e.target.value) })}
                 className="mt-1 w-full bg-slate-800 rounded p-1.5" />
        </label>
        <label className="block text-xs">표시 통화
          <input value={draft.display_currency}
                 onChange={(e) => setDraft({ ...draft, display_currency: e.target.value.toUpperCase() })}
                 className="mt-1 w-full bg-slate-800 rounded p-1.5" />
        </label>
        <label className="block text-xs">위젯 투명도 ({draft.widget_opacity.toFixed(2)})
          <input type="range" min={0.1} max={1} step={0.05} value={draft.widget_opacity}
                 onChange={(e) => setDraft({ ...draft, widget_opacity: Number(e.target.value) })}
                 className="mt-1 w-full" />
        </label>
        <div className="border-t border-slate-800 pt-3">
          <div className="text-xs uppercase text-slate-400 mb-2">AI API 키 (BYOK)</div>
          <div className="flex gap-2">
            <select value={keyDraft.provider} onChange={(e) => setKeyDraft({ ...keyDraft, provider: e.target.value as AiProviderKind })} className="bg-slate-800 rounded p-1.5 text-xs">
              <option value="openai">OpenAI</option>
              <option value="anthropic">Anthropic</option>
              <option value="gemini">Gemini</option>
            </select>
            <input type="password" value={keyDraft.key} onChange={(e) => setKeyDraft({ ...keyDraft, key: e.target.value })}
                   placeholder="sk-..." className="flex-1 bg-slate-800 rounded p-1.5 text-xs" />
            <button type="button" onClick={saveKey} className="bg-emerald-600 rounded px-3 text-xs">저장</button>
            <button type="button" onClick={clearKey} className="bg-rose-900 rounded px-3 text-xs">삭제</button>
          </div>
          <p className="text-[10px] text-slate-500 mt-1">키는 OS 키체인에 암호화 저장됨</p>
        </div>
        <div className="flex gap-2 justify-end">
          <button onClick={onClose} className="px-3 py-1 text-sm rounded bg-slate-800">취소</button>
          <button onClick={commit} className="px-3 py-1 text-sm rounded bg-emerald-600">저장</button>
        </div>
      </div>
    </div>
  );
}

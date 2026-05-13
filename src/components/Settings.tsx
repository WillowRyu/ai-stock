import { useEffect, useState } from "react";
import { useSettingsStore } from "../lib/state/settingsStore";
import { useThemeStore, type ThemeMode } from "../lib/state/themeStore";
import { aiIpc, kisIpc, type AiProviderKind, type AppSettingsDto } from "../lib/ipc";

const THEMES: { value: ThemeMode; label: string }[] = [
  { value: "light", label: "라이트" },
  { value: "dark", label: "다크" },
  { value: "system", label: "시스템" },
];

export function Settings({ onClose }: { onClose(): void }) {
  const { load, save } = useSettingsStore();
  const themeMode = useThemeStore((s) => s.mode);
  const setThemeMode = useThemeStore((s) => s.setMode);
  const [draft, setDraft] = useState<AppSettingsDto | null>(null);
  const [keyDraft, setKeyDraft] = useState<{ provider: AiProviderKind; key: string }>({ provider: "openai", key: "" });
  const [kisHas, setKisHas] = useState(false);
  const [kisDraft, setKisDraft] = useState({ app_key: "", app_secret: "" });
  useEffect(() => { load().then(() => setDraft(useSettingsStore.getState().settings)); }, [load]);
  useEffect(() => { kisIpc.has().then(setKisHas); }, []);
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

  async function saveKis() {
    if (!kisDraft.app_key || !kisDraft.app_secret) return;
    await kisIpc.setCredentials(kisDraft.app_key, kisDraft.app_secret);
    setKisDraft({ app_key: "", app_secret: "" });
    setKisHas(true);
  }
  async function clearKis() {
    await kisIpc.clear();
    setKisHas(false);
  }

  return (
    <div className="fixed inset-0 z-50 bg-black/50 flex items-center justify-center" onClick={onClose}>
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
          <div className="text-xs uppercase text-slate-400 mb-2">테마</div>
          <div className="flex gap-2 text-sm">
            {THEMES.map((t) => (
              <button
                key={t.value}
                type="button"
                onClick={() => setThemeMode(t.value)}
                className={
                  "px-3 py-2 rounded border " +
                  (themeMode === t.value
                    ? "bg-emerald-600 border-emerald-500 text-white"
                    : "bg-slate-800 border-slate-700 hover:bg-slate-700")
                }
              >
                {t.label}
              </button>
            ))}
          </div>
        </div>
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
        <div className="border-t border-slate-800 pt-3">
          <div className="text-xs uppercase text-slate-400 mb-2">한국투자 OpenAPI (KR 주식)</div>
          <p className="text-[10px] text-slate-500 mb-2">
            https://apiportal.koreainvestment.com 에서 앱 등록 후 발급받은 app_key/app_secret을 입력하세요. 입력 시 한국 주식은 KIS API에서, 미설정 시 Naver 스크래핑으로 대체합니다.
          </p>
          <div className="grid grid-cols-2 gap-2 text-xs">
            <input
              type="text"
              value={kisDraft.app_key}
              onChange={(e) => setKisDraft({ ...kisDraft, app_key: e.target.value })}
              placeholder="app_key"
              className="bg-slate-800 rounded p-2 text-sm col-span-2"
            />
            <input
              type="password"
              value={kisDraft.app_secret}
              onChange={(e) => setKisDraft({ ...kisDraft, app_secret: e.target.value })}
              placeholder="app_secret"
              className="bg-slate-800 rounded p-2 text-sm col-span-2"
            />
            <div className="col-span-2 flex items-center gap-2">
              <span className={kisHas ? "text-emerald-400 text-xs" : "text-slate-500 text-xs"}>
                {kisHas ? "키 설정됨" : "키 없음"}
              </span>
              <button type="button" onClick={saveKis} className="ml-auto bg-emerald-600 hover:bg-emerald-500 rounded px-3 text-xs py-1.5">저장</button>
              <button type="button" onClick={clearKis} className="bg-rose-900 hover:bg-rose-800 rounded px-3 text-xs py-1.5">삭제</button>
            </div>
          </div>
        </div>
        <div className="flex gap-2 justify-end">
          <button onClick={onClose} className="px-3 py-1 text-sm rounded bg-slate-800">취소</button>
          <button onClick={commit} className="px-3 py-1 text-sm rounded bg-emerald-600">저장</button>
        </div>
      </div>
    </div>
  );
}

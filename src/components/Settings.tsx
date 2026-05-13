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
    <div className="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center" onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} className="glass-panel rounded-lg p-5 w-96 space-y-3">
        <h3 className="text-lg font-semibold text-slate-900 dark:text-slate-100">설정</h3>
        <label className="block text-xs text-slate-700 dark:text-slate-300">폴링 주기 (초)
          <input type="number" min={1} max={300} value={draft.poll_interval_secs}
                 onChange={(e) => setDraft({ ...draft, poll_interval_secs: Number(e.target.value) })}
                 className="mt-1 w-full glass-inset rounded p-1.5 text-slate-900 dark:text-slate-100" />
        </label>
        <label className="block text-xs text-slate-700 dark:text-slate-300">표시 통화
          <input value={draft.display_currency}
                 onChange={(e) => setDraft({ ...draft, display_currency: e.target.value.toUpperCase() })}
                 className="mt-1 w-full glass-inset rounded p-1.5 text-slate-900 dark:text-slate-100" />
        </label>
        <label className="block text-xs text-slate-700 dark:text-slate-300">위젯 투명도 ({draft.widget_opacity.toFixed(2)})
          <input type="range" min={0.1} max={1} step={0.05} value={draft.widget_opacity}
                 onChange={(e) => setDraft({ ...draft, widget_opacity: Number(e.target.value) })}
                 className="mt-1 w-full" />
        </label>
        <div className="border-t border-slate-300/40 dark:border-white/10 pt-3">
          <div className="text-xs uppercase text-slate-500 dark:text-slate-400 mb-2">테마</div>
          <div className="flex gap-2 text-sm">
            {THEMES.map((t) => (
              <button
                key={t.value}
                type="button"
                onClick={() => setThemeMode(t.value)}
                className={
                  themeMode === t.value
                    ? "px-3 py-2 rounded border bg-emerald-600 border-emerald-500 text-white"
                    : "btn-secondary"
                }
              >
                {t.label}
              </button>
            ))}
          </div>
        </div>
        <div className="border-t border-slate-300/40 dark:border-white/10 pt-3">
          <div className="text-xs uppercase text-slate-500 dark:text-slate-400 mb-2">AI API 키 (BYOK)</div>
          <div className="flex gap-2">
            <select value={keyDraft.provider} onChange={(e) => setKeyDraft({ ...keyDraft, provider: e.target.value as AiProviderKind })} className="glass-inset rounded p-1.5 text-xs text-slate-700 dark:text-slate-200">
              <option value="openai">OpenAI</option>
              <option value="anthropic">Anthropic</option>
              <option value="gemini">Gemini</option>
            </select>
            <input type="password" value={keyDraft.key} onChange={(e) => setKeyDraft({ ...keyDraft, key: e.target.value })}
                   placeholder="sk-..." className="flex-1 glass-inset rounded p-1.5 text-xs text-slate-900 dark:text-slate-100" />
            <button type="button" onClick={saveKey} className="btn-primary text-xs px-3 py-1.5">저장</button>
            <button type="button" onClick={clearKey} className="bg-rose-700 hover:bg-rose-600 text-white rounded px-3 text-xs py-1.5">삭제</button>
          </div>
          <p className="text-[10px] text-slate-500 dark:text-slate-500 mt-1">키는 OS 키체인에 암호화 저장됨</p>
        </div>
        <div className="border-t border-slate-300/40 dark:border-white/10 pt-3">
          <div className="text-xs uppercase text-slate-500 dark:text-slate-400 mb-2">한국투자 OpenAPI (KR 주식)</div>
          <p className="text-[10px] text-slate-500 dark:text-slate-500 mb-2">
            https://apiportal.koreainvestment.com 에서 앱 등록 후 발급받은 app_key/app_secret을 입력하세요. 입력 시 한국 주식은 KIS API에서, 미설정 시 Naver 스크래핑으로 대체합니다.
          </p>
          <div className="grid grid-cols-2 gap-2 text-xs">
            <input
              type="text"
              value={kisDraft.app_key}
              onChange={(e) => setKisDraft({ ...kisDraft, app_key: e.target.value })}
              placeholder="app_key"
              className="glass-inset rounded p-2 text-sm col-span-2 text-slate-900 dark:text-slate-100"
            />
            <input
              type="password"
              value={kisDraft.app_secret}
              onChange={(e) => setKisDraft({ ...kisDraft, app_secret: e.target.value })}
              placeholder="app_secret"
              className="glass-inset rounded p-2 text-sm col-span-2 text-slate-900 dark:text-slate-100"
            />
            <div className="col-span-2 flex items-center gap-2">
              <span className={kisHas ? "text-emerald-600 dark:text-emerald-400 text-xs" : "text-slate-500 dark:text-slate-500 text-xs"}>
                {kisHas ? "키 설정됨" : "키 없음"}
              </span>
              <button type="button" onClick={saveKis} className="ml-auto btn-primary text-xs px-3 py-1.5">저장</button>
              <button type="button" onClick={clearKis} className="bg-rose-700 hover:bg-rose-600 text-white rounded px-3 text-xs py-1.5">삭제</button>
            </div>
          </div>
        </div>
        <div className="flex gap-2 justify-end">
          <button onClick={onClose} className="btn-secondary text-sm">취소</button>
          <button onClick={commit} className="btn-primary text-sm">저장</button>
        </div>
      </div>
    </div>
  );
}

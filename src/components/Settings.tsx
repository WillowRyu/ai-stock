import { useEffect, useState, type ReactNode } from "react";
import { useSettingsStore } from "../lib/state/settingsStore";
import { useThemeStore, type ThemeMode } from "../lib/state/themeStore";
import { aiIpc, kisIpc, type AiProviderKind, type AppSettingsDto } from "../lib/ipc";

const THEMES: { value: ThemeMode; label: string; hint: string }[] = [
  { value: "light", label: "라이트", hint: "밝은 배경" },
  { value: "dark", label: "다크", hint: "어두운 배경" },
  { value: "system", label: "시스템", hint: "OS 설정 따름" },
];

const AI_PROVIDERS: { value: AiProviderKind; label: string }[] = [
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "gemini", label: "Gemini" },
];

export function Settings({ onClose }: { onClose(): void }) {
  const { load, save } = useSettingsStore();
  const themeMode = useThemeStore((s) => s.mode);
  const setThemeMode = useThemeStore((s) => s.setMode);
  const [draft, setDraft] = useState<AppSettingsDto | null>(null);
  const [keyDraft, setKeyDraft] = useState<{ provider: AiProviderKind; key: string }>({
    provider: "openai",
    key: "",
  });
  const [aiHas, setAiHas] = useState<Record<AiProviderKind, boolean>>({
    openai: false,
    anthropic: false,
    gemini: false,
  });
  const [kisHas, setKisHas] = useState(false);
  const [kisDraft, setKisDraft] = useState({ app_key: "", app_secret: "" });
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    load().then(() => setDraft(useSettingsStore.getState().settings));
  }, [load]);

  useEffect(() => {
    kisIpc.has().then(setKisHas);
    Promise.all([
      aiIpc.hasKey("openai"),
      aiIpc.hasKey("anthropic"),
      aiIpc.hasKey("gemini"),
    ]).then(([o, a, g]) => setAiHas({ openai: o, anthropic: a, gemini: g }));
  }, []);

  if (!draft) return null;

  async function commit() {
    if (!draft) return;
    setSaving(true);
    try {
      await save(draft);
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
    } finally {
      setSaving(false);
    }
  }

  async function saveAiKey() {
    if (!keyDraft.key) return;
    await aiIpc.setKey(keyDraft.provider, keyDraft.key);
    setKeyDraft({ ...keyDraft, key: "" });
    setAiHas({ ...aiHas, [keyDraft.provider]: true });
  }
  async function clearAiKey() {
    await aiIpc.clearKey(keyDraft.provider);
    setAiHas({ ...aiHas, [keyDraft.provider]: false });
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
    <div
      className="fixed inset-0 z-50 bg-black/40 backdrop-blur-sm flex items-center justify-center p-4"
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        className="glass-panel rounded-xl w-full max-w-2xl max-h-[88vh] flex flex-col overflow-hidden"
      >
        {/* Sticky header */}
        <div className="px-6 py-4 border-b border-slate-300/40 dark:border-white/10 flex items-center justify-between">
          <h2 className="text-lg font-semibold text-slate-900 dark:text-slate-100">설정</h2>
          <button
            onClick={onClose}
            aria-label="닫기"
            className="w-7 h-7 rounded-full flex items-center justify-center text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 hover:bg-slate-200/50 dark:hover:bg-white/10"
          >
            ×
          </button>
        </div>

        {/* Scrollable body */}
        <div className="flex-1 overflow-y-auto px-6 py-5 space-y-7">
          {/* Appearance */}
          <Section title="화면" description="앱 외관과 표시 통화를 정합니다.">
            <Field label="테마">
              <div className="grid grid-cols-3 gap-2">
                {THEMES.map((t) => {
                  const active = themeMode === t.value;
                  return (
                    <button
                      key={t.value}
                      type="button"
                      onClick={() => setThemeMode(t.value)}
                      className={
                        "rounded-lg border px-3 py-3 text-left transition-colors " +
                        (active
                          ? "bg-emerald-600 border-emerald-500 text-white shadow-sm"
                          : "glass-inset border-slate-300/40 dark:border-white/10 hover:bg-white/40 dark:hover:bg-white/5 text-slate-700 dark:text-slate-200")
                      }
                    >
                      <div className="text-sm font-medium">{t.label}</div>
                      <div className={"text-[11px] mt-0.5 " + (active ? "text-emerald-50/90" : "text-slate-500 dark:text-slate-400")}>
                        {t.hint}
                      </div>
                    </button>
                  );
                })}
              </div>
            </Field>

            <Field label="표시 통화" hint="포트폴리오 합산에 사용. ISO 통화 코드 (예: USD, KRW).">
              <input
                value={draft.display_currency}
                onChange={(e) => setDraft({ ...draft, display_currency: e.target.value.toUpperCase() })}
                className="w-full glass-inset rounded-lg px-3 py-2.5 text-base text-slate-900 dark:text-slate-100"
              />
            </Field>
          </Section>

          {/* Data */}
          <Section title="데이터" description="실시간 갱신 주기를 조절합니다.">
            <Field label={`폴링 주기 — ${draft.poll_interval_secs}초마다`} hint="1~300초. 자주 갱신할수록 API 부하가 늘어납니다.">
              <input
                type="range"
                min={1}
                max={60}
                value={draft.poll_interval_secs}
                onChange={(e) => setDraft({ ...draft, poll_interval_secs: Number(e.target.value) })}
                className="w-full accent-emerald-500"
              />
              <div className="flex justify-between text-[10px] text-slate-500 dark:text-slate-500 mt-1">
                <span>1초</span><span>30초</span><span>60초</span>
              </div>
            </Field>
          </Section>

          {/* Widget */}
          <Section title="위젯" description="플로팅 미니 창의 외관.">
            <Field label={`투명도 — ${(draft.widget_opacity * 100).toFixed(0)}%`}>
              <input
                type="range"
                min={0.1}
                max={1}
                step={0.05}
                value={draft.widget_opacity}
                onChange={(e) => setDraft({ ...draft, widget_opacity: Number(e.target.value) })}
                className="w-full accent-emerald-500"
              />
            </Field>
          </Section>

          {/* AI BYOK */}
          <Section
            title="AI API 키 (BYOK)"
            description="키는 OS 키체인에 암호화 저장됩니다. 미설정 시 AI 기능이 비활성화됩니다."
          >
            <Field label="공급자">
              <div className="grid grid-cols-3 gap-2">
                {AI_PROVIDERS.map((p) => {
                  const active = keyDraft.provider === p.value;
                  const has = aiHas[p.value];
                  return (
                    <button
                      key={p.value}
                      type="button"
                      onClick={() => setKeyDraft({ ...keyDraft, provider: p.value, key: "" })}
                      className={
                        "rounded-lg border px-3 py-2.5 text-left transition-colors flex items-center justify-between " +
                        (active
                          ? "bg-emerald-600 border-emerald-500 text-white shadow-sm"
                          : "glass-inset border-slate-300/40 dark:border-white/10 hover:bg-white/40 dark:hover:bg-white/5 text-slate-700 dark:text-slate-200")
                      }
                    >
                      <span className="text-sm font-medium">{p.label}</span>
                      <span
                        className={
                          "w-1.5 h-1.5 rounded-full " +
                          (has ? (active ? "bg-emerald-100" : "bg-emerald-500") : (active ? "bg-emerald-300/60" : "bg-slate-400 dark:bg-slate-600"))
                        }
                        title={has ? "키 설정됨" : "키 없음"}
                      />
                    </button>
                  );
                })}
              </div>
            </Field>

            <Field label={`${AI_PROVIDERS.find((p) => p.value === keyDraft.provider)?.label} API 키`}>
              <div className="flex gap-2">
                <input
                  type="password"
                  value={keyDraft.key}
                  onChange={(e) => setKeyDraft({ ...keyDraft, key: e.target.value })}
                  placeholder={aiHas[keyDraft.provider] ? "••••••••  (저장된 키 있음. 덮어쓰려면 새 키 입력)" : "sk-..."}
                  className="flex-1 min-w-0 glass-inset rounded-lg px-3 py-2.5 text-base text-slate-900 dark:text-slate-100"
                />
                <button type="button" onClick={saveAiKey} disabled={!keyDraft.key} className="btn-primary disabled:opacity-50">
                  저장
                </button>
                <button
                  type="button"
                  onClick={clearAiKey}
                  disabled={!aiHas[keyDraft.provider]}
                  className="rounded-lg px-3 py-2 text-sm text-rose-700 dark:text-rose-300 hover:bg-rose-500/10 disabled:opacity-30 disabled:cursor-not-allowed"
                >
                  삭제
                </button>
              </div>
            </Field>
          </Section>

          {/* KIS */}
          <Section
            title="한국투자 OpenAPI"
            description={
              <>
                <span>
                  한국 주식 데이터를 KIS OpenAPI로 받습니다. 미설정 시 Naver 스크래핑으로 자동 대체.
                </span>
                <br />
                <span className="text-slate-400 dark:text-slate-500">
                  https://apiportal.koreainvestment.com 에서 앱 등록 후 발급받은 키를 입력하세요.
                </span>
              </>
            }
          >
            <Field label="app_key">
              <input
                type="text"
                value={kisDraft.app_key}
                onChange={(e) => setKisDraft({ ...kisDraft, app_key: e.target.value })}
                placeholder={kisHas ? "••••••••  (저장된 키 있음)" : "PSXXXXX..."}
                className="w-full glass-inset rounded-lg px-3 py-2.5 text-base text-slate-900 dark:text-slate-100"
              />
            </Field>
            <Field label="app_secret">
              <input
                type="password"
                value={kisDraft.app_secret}
                onChange={(e) => setKisDraft({ ...kisDraft, app_secret: e.target.value })}
                placeholder={kisHas ? "••••••••" : ""}
                className="w-full glass-inset rounded-lg px-3 py-2.5 text-base text-slate-900 dark:text-slate-100"
              />
            </Field>
            <div className="flex items-center gap-2">
              <span
                className={
                  "text-xs flex items-center gap-1.5 " +
                  (kisHas ? "text-emerald-600 dark:text-emerald-400" : "text-slate-500 dark:text-slate-400")
                }
              >
                <span className={"w-1.5 h-1.5 rounded-full " + (kisHas ? "bg-emerald-500" : "bg-slate-400 dark:bg-slate-600")} />
                {kisHas ? "키 설정됨 — KIS API 사용 중" : "키 없음 — Naver 폴백 사용 중"}
              </span>
              <div className="ml-auto flex gap-2">
                <button
                  type="button"
                  onClick={saveKis}
                  disabled={!kisDraft.app_key || !kisDraft.app_secret}
                  className="btn-primary disabled:opacity-50"
                >
                  저장
                </button>
                <button
                  type="button"
                  onClick={clearKis}
                  disabled={!kisHas}
                  className="rounded-lg px-3 py-2 text-sm text-rose-700 dark:text-rose-300 hover:bg-rose-500/10 disabled:opacity-30 disabled:cursor-not-allowed"
                >
                  삭제
                </button>
              </div>
            </div>
          </Section>
        </div>

        {/* Sticky footer */}
        <div className="px-6 py-4 border-t border-slate-300/40 dark:border-white/10 flex items-center justify-end gap-2">
          {saved && (
            <span className="text-xs text-emerald-600 dark:text-emerald-400 mr-auto">저장됨</span>
          )}
          <button onClick={onClose} className="btn-secondary">취소</button>
          <button onClick={commit} disabled={saving} className="btn-primary disabled:opacity-50">
            {saving ? "저장 중..." : "저장"}
          </button>
        </div>
      </div>
    </div>
  );
}

function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="space-y-3">
      <div>
        <h3 className="text-sm font-semibold text-slate-900 dark:text-slate-100">{title}</h3>
        {description && (
          <p className="text-[11px] text-slate-500 dark:text-slate-400 mt-0.5 leading-relaxed">
            {description}
          </p>
        )}
      </div>
      <div className="space-y-3">{children}</div>
    </section>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <div className="text-xs font-medium text-slate-700 dark:text-slate-300">{label}</div>
      {children}
      {hint && <div className="text-[11px] text-slate-500 dark:text-slate-500">{hint}</div>}
    </div>
  );
}

import { create } from "zustand";

export type ThemeMode = "light" | "dark" | "system";

function isSystemDark(): boolean {
  if (typeof window === "undefined") return true;
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function effectiveDark(mode: ThemeMode): boolean {
  return mode === "dark" || (mode === "system" && isSystemDark());
}

function applyDarkClass(dark: boolean) {
  if (typeof document === "undefined") return;
  document.documentElement.classList.toggle("dark", dark);
}

interface ThemeState {
  mode: ThemeMode;
  isDark: boolean;
  setMode(m: ThemeMode): void;
}

const initialMode: ThemeMode = ((): ThemeMode => {
  try {
    const v = localStorage.getItem("theme") as ThemeMode | null;
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    /* ignore */
  }
  return "system";
})();

const initialDark = effectiveDark(initialMode);
applyDarkClass(initialDark);

// Re-apply on system theme change when in "system" mode.
if (typeof window !== "undefined") {
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    const { mode } = useThemeStore.getState();
    if (mode === "system") {
      const d = isSystemDark();
      applyDarkClass(d);
      useThemeStore.setState({ isDark: d });
    }
  });
}

export const useThemeStore = create<ThemeState>((set) => ({
  mode: initialMode,
  isDark: initialDark,
  setMode(m) {
    try {
      localStorage.setItem("theme", m);
    } catch {
      /* ignore */
    }
    const d = effectiveDark(m);
    applyDarkClass(d);
    set({ mode: m, isDark: d });
  },
}));

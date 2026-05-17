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

// Sync the native window appearance (macOS NSWindow.appearance) so the OS
// vibrancy material renders light/dark to match the in-app theme. Without
// this, picking the in-app light theme while the OS is in dark mode leaves a
// dark vibrancy backdrop behind light-theme (dark) text — unreadable.
function applyNativeTheme(dark: boolean) {
  import("@tauri-apps/api/core")
    .then(({ invoke }) => invoke("set_window_theme", { dark }))
    .catch(() => {
      /* not running inside Tauri (e.g. unit tests) — ignore */
    });
}

function applyTheme(dark: boolean) {
  applyDarkClass(dark);
  applyNativeTheme(dark);
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
applyTheme(initialDark);

// Re-apply on system theme change when in "system" mode.
if (typeof window !== "undefined") {
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    const { mode } = useThemeStore.getState();
    if (mode === "system") {
      const d = isSystemDark();
      applyTheme(d);
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
    applyTheme(d);
    set({ mode: m, isDark: d });
  },
}));

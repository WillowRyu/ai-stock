import { create } from "zustand";
import { ipc, type AppSettingsDto } from "../ipc";

interface SettingsState {
  settings: AppSettingsDto | null;
  load(): Promise<void>;
  save(s: AppSettingsDto): Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: null,
  async load() { set({ settings: await ipc.settingsGet() }); },
  async save(s) { await ipc.settingsSave(s); set({ settings: s }); },
}));

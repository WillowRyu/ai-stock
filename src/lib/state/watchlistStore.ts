import { create } from "zustand";
import { ipc, type SymbolDto } from "../ipc";

interface WatchlistState {
  symbols: SymbolDto[];
  loading: boolean;
  load(): Promise<void>;
  add(s: SymbolDto): Promise<void>;
  remove(s: SymbolDto): Promise<void>;
}

export const useWatchlistStore = create<WatchlistState>((set) => ({
  symbols: [],
  loading: false,
  async load() {
    set({ loading: true });
    try { set({ symbols: await ipc.watchlistGet() }); } finally { set({ loading: false }); }
  },
  async add(s) {
    await ipc.watchlistAdd(s);
    set((p) => ({ symbols: [...p.symbols.filter((x) => !sameSymbol(x, s)), s] }));
  },
  async remove(s) {
    await ipc.watchlistRemove(s);
    set((p) => ({ symbols: p.symbols.filter((x) => !sameSymbol(x, s)) }));
  },
}));

function sameSymbol(a: SymbolDto, b: SymbolDto) {
  return a.kind === b.kind && a.ticker === b.ticker && (a.quote_currency ?? null) === (b.quote_currency ?? null);
}

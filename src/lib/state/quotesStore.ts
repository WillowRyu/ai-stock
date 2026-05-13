import { create } from "zustand";
import type { QuoteDto, SymbolDto } from "../ipc";

function key(s: SymbolDto): string {
  return s.quote_currency ? `${s.kind}:${s.ticker}:${s.quote_currency}` : `${s.kind}:${s.ticker}`;
}

interface QuotesState {
  bySymbol: Record<string, QuoteDto>;
  apply(updates: QuoteDto[]): void;
}

export const useQuotesStore = create<QuotesState>((set) => ({
  bySymbol: {},
  apply(updates) {
    set((prev) => {
      const next = { ...prev.bySymbol };
      for (const q of updates) next[key(q.symbol)] = q;
      return { bySymbol: next };
    });
  },
}));

export function quoteKey(s: SymbolDto): string { return key(s); }

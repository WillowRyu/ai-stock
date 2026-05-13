import { create } from "zustand";
import { ipc, type HoldingDto, type PortfolioValuationDto, type SymbolDto } from "../ipc";

interface PortfolioState {
  valuation: PortfolioValuationDto | null;
  refresh(): Promise<void>;
  upsert(h: HoldingDto): Promise<void>;
  remove(s: SymbolDto): Promise<void>;
}

export const usePortfolioStore = create<PortfolioState>((set) => ({
  valuation: null,
  async refresh() { set({ valuation: await ipc.portfolioValuation() }); },
  async upsert(h) { await ipc.portfolioUpsert(h); set({ valuation: await ipc.portfolioValuation() }); },
  async remove(s) { await ipc.portfolioDelete(s); set({ valuation: await ipc.portfolioValuation() }); },
}));

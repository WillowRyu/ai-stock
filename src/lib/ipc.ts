import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type AssetKind = "crypto" | "us" | "kr" | "fx" | "com";

export interface SymbolDto {
  kind: AssetKind;
  ticker: string;
  quote_currency?: string | null;
}

export interface QuoteDto {
  symbol: SymbolDto;
  price: string;       // decimal-as-string
  currency: string;
  change_24h: string | null;
  observed_at: string; // RFC3339
}

export interface HoldingDto {
  symbol: SymbolDto;
  quantity: string;
  avg_cost_amount: string;
  avg_cost_currency: string;
}

export interface HoldingValuationDto {
  symbol: SymbolDto;
  market_value: string | null;
  cost_basis: string;
  pnl: string | null;
}

export interface PortfolioValuationDto {
  total_value: string | null;
  total_value_currency: string | null;
  total_pnl: string | null;
  holdings: HoldingValuationDto[];
}

export interface AppSettingsDto {
  poll_interval_secs: number;
  display_currency: string;
  theme: string;
  widget_opacity: number;
  widget_always_on_top: boolean;
}

export const ipc = {
  watchlistGet: () => invoke<SymbolDto[]>("watchlist_get"),
  watchlistAdd: (symbol: SymbolDto) => invoke<void>("watchlist_add", { symbol }),
  watchlistRemove: (symbol: SymbolDto) => invoke<void>("watchlist_remove", { symbol }),

  quotesSnapshot: () => invoke<QuoteDto[]>("quotes_snapshot"),

  portfolioUpsert: (holding: HoldingDto) => invoke<void>("portfolio_upsert", { holding }),
  portfolioDelete: (symbol: SymbolDto) => invoke<void>("portfolio_delete", { symbol }),
  portfolioValuation: () => invoke<PortfolioValuationDto>("portfolio_valuation"),

  settingsGet: () => invoke<AppSettingsDto>("settings_get"),
  settingsSave: (settings: AppSettingsDto) => invoke<void>("settings_save", { settings }),

  widgetToggle: () => invoke<void>("widget_toggle"),
};

export function onQuoteUpdate(cb: (quotes: QuoteDto[]) => void): Promise<UnlistenFn> {
  return listen<QuoteDto[]>("quote-update", (e) => cb(e.payload));
}

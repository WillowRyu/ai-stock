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
  display_name: string | null;
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

export type AlertConditionKind = "above" | "below" | "rsi_above" | "rsi_below" | "macd_golden" | "macd_death";

export interface AlertRuleDto {
  id: number;
  symbol: SymbolDto;
  condition: AlertConditionKind;
  threshold_amount: string | null;       // null for macd_*
  threshold_currency: string | null;     // null for rsi_*, macd_*
  enabled: boolean;
  cooldown_secs: number;
}

export const alertsIpc = {
  list: () => invoke<AlertRuleDto[]>("alerts_list"),
  create: (rule: AlertRuleDto) => invoke<number>("alerts_create", { rule }),
  delete: (id: number) => invoke<void>("alerts_delete", { id }),
};

export type AiProviderKind = "openai" | "anthropic" | "gemini";
export type AiPromptKind = "commentary" | "chart_analysis" | "news_summary";

export const aiIpc = {
  setKey: (provider: AiProviderKind, key: string) =>
    invoke<void>("ai_set_key", { provider, key }),
  clearKey: (provider: AiProviderKind) => invoke<void>("ai_clear_key", { provider }),
  hasKey: (provider: AiProviderKind) => invoke<boolean>("ai_has_key", { provider }),
  startTurn: (provider: AiProviderKind, symbol: SymbolDto, kind: AiPromptKind) =>
    invoke<void>("ai_start_turn", { provider, symbol, kind }),
  sendMessage: (provider: AiProviderKind, symbol: SymbolDto, text: string) =>
    invoke<void>("ai_send_message", { provider, symbol, text }),
  cancel: () => invoke<void>("ai_cancel"),
};

export const kisIpc = {
  setCredentials: (app_key: string, app_secret: string) =>
    invoke<void>("kis_set_credentials", { app_key, app_secret }),
  clear: () => invoke<void>("kis_clear_credentials"),
  has: () => invoke<boolean>("kis_has_credentials"),
};

export function onAiChunk(cb: (text: string) => void): Promise<UnlistenFn> {
  return listen<string>("ai-chunk", (e) => cb(e.payload));
}
export function onAiDone(cb: () => void): Promise<UnlistenFn> {
  return listen<null>("ai-done", () => cb());
}
export function onAiError(cb: (msg: string) => void): Promise<UnlistenFn> {
  return listen<string>("ai-error", (e) => cb(e.payload));
}

export interface CandleDto {
  opened_at: string;
  open: string;
  high: string;
  low: string;
  close: string;
  volume: string;
}

export interface ChartDataDto {
  candles: CandleDto[];
  sma_20: (string | null)[];
  sma_50: (string | null)[];
  rsi_14: (string | null)[];
  macd: (string | null)[];
  macd_signal: (string | null)[];
  macd_histogram: (string | null)[];
}

export type CandleInterval = "1m" | "5m" | "15m" | "30m" | "1h" | "1d" | "1w";

export const chartIpc = {
  fetch: (symbol: SymbolDto, days: number, interval: CandleInterval = "1d") =>
    invoke<ChartDataDto>("chart_data", { symbol, days, interval }),
};

export interface ProviderErrorDto {
  symbol_canonical: string;
  provider: string;
  error: string;
}

export function onProviderError(cb: (e: ProviderErrorDto) => void): Promise<UnlistenFn> {
  return listen<ProviderErrorDto>("provider-error", (e) => cb(e.payload));
}

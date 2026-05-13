import { describe, it, expect } from "vitest";
import { useQuotesStore } from "./quotesStore";

describe("quotesStore", () => {
  it("apply merges by symbol key", () => {
    const { apply } = useQuotesStore.getState();
    apply([{
      symbol: { kind: "crypto", ticker: "BTC", quote_currency: "USD" },
      price: "67000", currency: "USD", change_24h: "0.0124",
      observed_at: new Date().toISOString(),
    }]);
    const snap = useQuotesStore.getState().bySymbol;
    expect(snap["crypto:BTC:USD"].price).toBe("67000");
  });
});

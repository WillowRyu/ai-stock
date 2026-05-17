import { describe, it, expect, beforeEach } from "vitest";
import { useAiStore } from "./aiStore";

describe("aiStore", () => {
  beforeEach(() => {
    useAiStore.setState({ bySymbol: {}, streaming: false });
  });

  it("accumulates a user turn then a streamed assistant reply", () => {
    const s = useAiStore.getState();
    s.pushUser("crypto:BTC:USD", "hello");
    s.startAssistant("crypto:BTC:USD");
    s.appendChunk("crypto:BTC:USD", "hi");
    s.appendChunk("crypto:BTC:USD", " there");
    s.finishStreaming();

    const msgs = useAiStore.getState().bySymbol["crypto:BTC:USD"];
    expect(msgs).toEqual([
      { role: "user", content: "hello" },
      { role: "assistant", content: "hi there" },
    ]);
    expect(useAiStore.getState().streaming).toBe(false);
  });

  it("keeps conversations separate per symbol key", () => {
    const s = useAiStore.getState();
    s.pushUser("crypto:BTC:USD", "btc?");
    s.pushUser("us:AAPL", "aapl?");
    expect(useAiStore.getState().bySymbol["crypto:BTC:USD"]).toHaveLength(1);
    expect(useAiStore.getState().bySymbol["us:AAPL"]).toHaveLength(1);
  });

  it("failStreaming drops a trailing empty assistant message", () => {
    const s = useAiStore.getState();
    s.pushUser("crypto:BTC:USD", "hello");
    s.startAssistant("crypto:BTC:USD");
    s.failStreaming("crypto:BTC:USD");
    const msgs = useAiStore.getState().bySymbol["crypto:BTC:USD"];
    expect(msgs).toEqual([{ role: "user", content: "hello" }]);
    expect(useAiStore.getState().streaming).toBe(false);
  });

  it("failStreaming keeps a partially-streamed assistant message", () => {
    const s = useAiStore.getState();
    s.pushUser("crypto:BTC:USD", "hello");
    s.startAssistant("crypto:BTC:USD");
    s.appendChunk("crypto:BTC:USD", "partial");
    s.failStreaming("crypto:BTC:USD");
    const msgs = useAiStore.getState().bySymbol["crypto:BTC:USD"];
    expect(msgs).toHaveLength(2);
    expect(msgs[1].content).toBe("partial");
  });
});

import { create } from "zustand";

export type AiRole = "user" | "assistant";

export interface AiMessage {
  role: AiRole;
  content: string;
}

interface AiState {
  /** Per-symbol message lists, keyed by `quoteKey(symbol)`. */
  bySymbol: Record<string, AiMessage[]>;
  streaming: boolean;
  pushUser(symKey: string, content: string): void;
  /** Push an empty assistant message and enter the streaming state. */
  startAssistant(symKey: string): void;
  /** Append a streamed chunk to the trailing assistant message. */
  appendChunk(symKey: string, text: string): void;
  finishStreaming(): void;
  /** End streaming and drop a trailing empty assistant message (error path). */
  failStreaming(symKey: string): void;
}

export const useAiStore = create<AiState>((set) => ({
  bySymbol: {},
  streaming: false,

  pushUser(symKey, content) {
    set((prev) => {
      const msgs = prev.bySymbol[symKey] ?? [];
      return {
        bySymbol: { ...prev.bySymbol, [symKey]: [...msgs, { role: "user", content }] },
      };
    });
  },

  startAssistant(symKey) {
    set((prev) => {
      const msgs = prev.bySymbol[symKey] ?? [];
      return {
        streaming: true,
        bySymbol: {
          ...prev.bySymbol,
          [symKey]: [...msgs, { role: "assistant", content: "" }],
        },
      };
    });
  },

  appendChunk(symKey, text) {
    set((prev) => {
      const msgs = prev.bySymbol[symKey] ?? [];
      if (msgs.length === 0) return prev;
      const last = msgs[msgs.length - 1];
      const updated = [...msgs.slice(0, -1), { ...last, content: last.content + text }];
      return { bySymbol: { ...prev.bySymbol, [symKey]: updated } };
    });
  },

  finishStreaming() {
    set({ streaming: false });
  },

  failStreaming(symKey) {
    set((prev) => {
      const msgs = prev.bySymbol[symKey] ?? [];
      const last = msgs[msgs.length - 1];
      if (last && last.role === "assistant" && last.content === "") {
        return {
          streaming: false,
          bySymbol: { ...prev.bySymbol, [symKey]: msgs.slice(0, -1) },
        };
      }
      return { streaming: false };
    });
  },
}));

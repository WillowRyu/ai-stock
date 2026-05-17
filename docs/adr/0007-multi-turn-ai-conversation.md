# ADR 0007 — Multi-turn AI conversation

- **Status:** Accepted
- **Date:** 2026-05-17 (M4)

## Context

M3 shipped a single-shot `commentary` call: one prompt in, one stream out, no
memory. ADR 0004 anticipated this would be extended ("Adding chat history
requires extending `AiPrompt` and the templates — straightforward"). M4 makes
the AI panel a per-symbol multi-turn chat.

Three API formats (OpenAI, Anthropic, Gemini) are all natively message-array
based, so the single `AiPrompt { system, user }` shape was the temporary
simplification, not the natural one.

## Decision

- A pure `Conversation` aggregate (`domain::conversation`) holds an ordered
  `Vec<Message>`; `Message` carries a `Role` (`User` / `Assistant`).
- The `AiProvider` port takes `AiRequest { system, messages, max_output_tokens }`
  instead of `AiPrompt`. `AiPrompt` is removed.
- `AiService` keeps per-symbol conversation state in memory
  (`HashMap<Symbol, SymbolChat>`), session-scoped — no persistence. It exposes
  `start_turn` (preset analysis), `send_message` (free-form follow-up), and
  `commit_assistant` (append the reply, full or partial-on-cancel).
- Context sent to the model is capped at the most recent `MAX_CONTEXT_MESSAGES`
  turns; the full conversation stays in memory.
- Cancellation is cooperative: the Tauri layer holds a `tokio::sync::watch`
  channel, `ai_cancel` flips it, and the chunk-pump loop exits — committing the
  partial assistant text so the next turn's context stays coherent.

## Consequences

- Each provider adapter maps `Role` to its wire format; Gemini needs
  `Assistant -> "model"`. A new provider is still one file.
- Conversation state is lost on app restart (session memory). Persisting it
  (SQLite) is deferred to M5+ — the design note in the M4 spec records why.
- The frontend `aiStore` mirrors turns for rendering; preset turns show a
  friendly label while the backend conversation holds the real data dump. The
  two stay structurally in sync (same turn count/order); assistant replies are
  identical.
- This ADR supersedes the single-shot assumption in ADR 0004.

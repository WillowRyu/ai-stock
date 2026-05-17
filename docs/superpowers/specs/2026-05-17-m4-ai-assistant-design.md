# M4 — Multi-turn AI Assistant — Design

- **Date:** 2026-05-17
- **Status:** Approved (brainstorming complete)
- **Milestone:** M4
- **Predecessors:** M1 (core), M2 (indicators/alerts/KR), M3 (BYOK AI), post-M3 polish.

## Summary

M4 extends the **AI Assistance** bounded context from a single-shot `commentary`
call into a per-symbol multi-turn conversation. The AI panel becomes a chat:
the user sends free-form messages, prior turns accumulate as context, and three
preset prompt types (`commentary`, `chart_analysis`, `news_summary`) are exposed
as quick-start buttons. Streaming generation can be cancelled.

This realizes the multi-turn direction ADR 0004 anticipated ("Adding chat history
requires extending `AiPrompt` and the templates — straightforward").

## Decisions (from brainstorming)

1. **Scope** — M4 is AI features only. 1.0 release prep (packaging, signing, CI,
   Naver contract test) is deferred to a separate M5 milestone with its own spec.
2. **Panel model** — conversational. Free-form user messages with accumulating
   context; the three prompt types are quick-start preset buttons, not the only
   interaction.
3. **History persistence** — session memory only, per-symbol. Cleared on app
   restart. No SQLite persistence in M4 (deferred to M5+).
4. **Cancellation** — explicit "Stop" button during generation, plus automatic
   cancellation on symbol switch / panel close.
5. **Architecture** — `Conversation` is a domain aggregate; the `AiProvider`
   trait carries the full message list each turn. All three API formats
   (OpenAI, Anthropic, Gemini) are natively message-array based.

## Out of Scope

- Persistent chat history (SQLite) — M5+.
- 1.0 release prep: installers, code signing, CI restoration, Naver contract
  test — M5.
- AI portfolio coaching — permanently deferred (per project overview).
- Cross-symbol / global chat — conversations are strictly per-symbol.

## Architecture

Layered changes, respecting DDD boundaries (`domain` ← `application` ←
`infrastructure` ← `app`/frontend):

- **domain** — new `Conversation` aggregate + `Message`/`Role` value objects;
  `prompt.rs` gains three prompt builders.
- **application** — `AiProvider` trait extended to multi-turn; `AiService` holds
  per-symbol conversation state and owns cancellation.
- **infrastructure** — the three adapters (`openai`, `anthropic`, `gemini`)
  rebuild their request bodies from a system string + message list.
- **app + frontend** — IPC commands for turns and cancellation; `AiPanel`
  rebuilt as a chat view backed by a new `aiStore`.

## Domain Layer

### `domain::conversation` (new module, pure — no IO)

- `Role` — `User` | `Assistant`. The system prompt is static per prompt type and
  is not stored as a `Message`.
- `Message { role: Role, content: String }`.
- `Conversation { messages: Vec<Message> }` with:
  - `new()`, `push_user(content)`, `push_assistant(content)`,
  - `messages() -> &[Message]`, `is_empty() -> bool`.

### `domain::prompt` (extended)

- `PromptKind` — `Commentary` | `ChartAnalysis` | `NewsSummary`.
- Three builders, each returning `(system, first_user_message)`:
  - `Commentary` — existing logic retained.
  - `ChartAnalysis` — centered on `IndicatorContext` (RSI, MACD, SMA crosses),
    emphasizes trend interpretation.
  - `NewsSummary` — centered on `headlines`, emphasizes summarizing them.
- The system prompt differs per kind (different role instructions). Missing-data
  handling follows the existing graceful pattern.
- Builders produce only the **first user message** of a conversation. Follow-up
  turns are free-form user input appended as `Message`s.

## Application Layer

### `AiProvider` trait change

- Current: `stream(AiPrompt { system, user, max_output_tokens })`.
- New: `stream(AiRequest { system: String, messages: Vec<Message>, max_output_tokens: u32 })`.
  `Message` is the domain type (application depends on domain — allowed).
- `AiPrompt` is removed. `AiChunk` (`Text` / `Done`) is unchanged.

### `AiService` change

- Holds per-symbol conversation state: `Mutex<HashMap<SymbolKey, Conversation>>`
  (session memory).
- Two methods:
  - `start_turn(provider_kind, symbol, kind: PromptKind)` — for quick-start
    buttons. Gathers context (quote / indicators / headlines), builds the first
    user message via the prompt builder, appends it to the conversation, streams.
  - `send_message(provider_kind, symbol, text)` — for free-form follow-ups.
    Appends the user message, streams.
- Both send `system + conversation.messages()` to the provider.
- Stream wrapping: as provider chunks pass through, assistant text is
  accumulated; on `Done`, the accumulated text is `push_assistant`-ed to the
  `Conversation`.
- History cap: the message list sent to the model is capped at the most recent
  N turns (~20 messages) to bound token usage. The full conversation stays in
  memory; only the transmitted slice is trimmed.

### Cancellation

- Each turn creates a `CancellationToken` (tokio-util), held in `AppState` as the
  "active turn" handle.
- The `ai_cancel` IPC command triggers the token; the chunk-forwarding loop exits.
- Partial assistant text accumulated up to the cancel point **is committed** to
  the conversation, so the next turn's context stays coherent.

## Infrastructure Layer

All three adapters convert `AiRequest` (system + message list) into their API's
format. The M3 SSE streaming parsers are unchanged — only request-body
construction changes.

| Adapter   | Mapping |
|-----------|---------|
| OpenAI    | `messages: [{role:"system"}, {role:"user"/"assistant"}, ...]` |
| Anthropic | top-level `system` field + `messages: [{role, content}]` |
| Gemini    | `systemInstruction` + `contents: [{role:"user"/"model"}]` — assistant maps to `"model"` |

Each adapter's wiremock test is updated to assert the multi-turn request body.

## IPC & Frontend

### IPC commands (`app/src/ipc.rs`)

- `ai_start_turn(provider, symbol, kind)` — replaces `ai_commentary`. `kind` is
  `"commentary"` | `"chart_analysis"` | `"news_summary"`.
- `ai_send_message(provider, symbol, text)` — free-form follow-up.
- `ai_cancel()` — cancels the active stream.
- `ai_set_key` / `ai_clear_key` / `ai_has_key` — unchanged.
- Events `ai-chunk` / `ai-done` / `ai-error` — unchanged.

### History synchronization

No `ai_history` command. The frontend `aiStore` mirrors per-symbol messages and
appends user messages / accumulates assistant chunks identically to the backend
`AiService`, so the two stay in sync by construction. On symbol switch, the panel
renders that symbol's conversation from the store.

### Frontend

- `aiStore` (new zustand store) — per-symbol `Message[]`, streaming state,
  selected provider.
- `AiPanel.tsx` rebuilt as a chat view:
  - message list (user / assistant bubbles),
  - three quick-start buttons — prominent when the conversation is empty, a
    toolbar once it has messages,
  - text input + send,
  - "Stop" button while streaming.
- Korean labels, following the existing i18n pattern.

## Testing Strategy (TDD red-green-refactor)

| Layer          | Tests |
|----------------|-------|
| domain         | `Conversation` push/ordering; each of the three prompt builders (chart-analysis includes indicators, news-summary includes headlines, missing data handled gracefully). Pure, no mocks. |
| application    | `AiService` multi-turn with a mock `AiProvider`: assistant reply accumulates into the conversation; `start_turn` vs `send_message`; cancellation commits partial text; history cap. |
| infrastructure | The three adapters via wiremock: multi-turn body, `system` field present, Gemini role mapping. |
| frontend       | `aiStore`: append user → accumulate chunks → done. |
| E2E            | Out of scope — no execution path (tauri-driver unsupported on macOS; CI removed). Revisit when CI is restored in M5. |

## Documentation Impact

- New ADR for the multi-turn `AiProvider` trait change and `Conversation`
  aggregate (supersedes the single-shot assumption recorded in ADR 0004).
- `docs/CONTEXT.md`: add `Conversation`, `Message`, `Role` to the ubiquitous
  language; update AI Assistance current-state.
- `docs/progress.md`: per-task entries through M4.

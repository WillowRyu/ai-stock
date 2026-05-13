# ADR 0004 — BYOK AI with streaming over Tauri events

- **Status:** Accepted
- **Date:** 2026-05-13 (M3)

## Context

The app needs optional AI commentary. We chose BYOK (user supplies an API key) for three reasons:

1. **Zero operating cost.** No hosted backend, no billing, no rate limiting on our side.
2. **Privacy.** API calls go directly from the user's machine to the provider; nothing routes through us.
3. **Provider choice.** Users pick the model they prefer (OpenAI / Anthropic / Gemini).

Three streaming formats had to be normalized: OpenAI SSE (`data: {...}`), Anthropic SSE with event types (`event: content_block_delta`), and Gemini's `streamGenerateContent?alt=sse` (similar to OpenAI). We unify behind an `AiProvider::stream(prompt) -> BoxStream<AiChunk>` trait.

Streaming flows through Tauri events (`ai-chunk` / `ai-done` / `ai-error`) rather than command return values so the UI can render tokens as they arrive without blocking the IPC roundtrip.

## Decision

- One `AiProvider` trait in `application::ports::ai_provider`; adapters in `infrastructure::providers::{openai, anthropic, gemini}`.
- One `PromptTemplate` pure function in `domain::prompt`. Context is composed in `AiService`.
- `AiService` is the only code that touches `SecretStore`. Keys are scoped per-provider.
- Frontend subscribes to `ai-chunk` and renders incrementally.
- AI is independent: with no key set, the app works exactly as M2.

## Consequences

- Each new provider is a single file + a `match` arm in the wiring factory.
- Adding chat history requires extending `AiPrompt` and the templates — straightforward.
- Switching to a hosted model later means a `HostedAiProvider` adapter, no changes elsewhere.
- BYOK puts the cost decision on the user. We never see their keys outside the keychain.

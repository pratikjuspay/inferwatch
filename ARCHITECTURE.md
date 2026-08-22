# Architecture

Deep-dive on how inferwatch is built and *why*. For the interactive file-level
map (mermaid dependency graph + sequence diagram), see [FILE_MAP.md](./FILE_MAP.md).

```text
┌──────────────────────────────┐        ┌──────────────────────────┐
│      SvelteKit frontend      │        │   ingest once, read many │
│  (SSE stream, convo list,    │        │        ┌─ PostgreSQL ─┐  │
│   live dashboard @5s)        │        │        │ conversations│  │
└──────────────┬───────────────┘        │ (worker)│ messages     │  │
               │  POST /api/chat/:id    │   └────►│inference_logs│  │
               │  response: text/e-v... │   ▲     └──────────────┘  │
┌──────────────▼────────────────────────┴───┴───────────────────────┐
│                        Axum backend (Rust)                         │
│                                                                    │
│  routes/chat ──► SDK (InstrumentedProvider) ──► LogEvent          │
│       │                  │                          │             │
│       │      ┌───────────┴────────────┐     tokio mpsc channel    │
│       │      │ LlmProvider (trait)    │          (10k buffer)     │
│       │      │ ├─ OpenAIProvider      │              │            │
│       │      │ └─ GeminiProvider      │              ▼            │
│       │      └────────────────────────┘      ingestion_worker     │
│       │                                          (spawn, 1)       │
│       └─ SSE events (token/done/error) → browser                │
└────────────────────────────────────────────────────────────────────┘
```

### Ingestion flow

1. `POST /api/chat/:id` verifies session, saves user message, loads last-20 history.
2. Handler calls **`sdk.complete()`** — application code never logs anything.
3. SDK times the call, accumulates the output, and fires **one** `LogEvent` into a tokio mpsc channel via `try_send` on completion (success *or* failure).
4. A separate `tokio::spawn`ed worker drains the channel and writes to `inference_logs`.
5. DB writes happen entirely off the request path — user latency = LLM latency only.

### Logging strategy (auto-instrumentation)

The SDK is the toll-booth camera, not a form the driver fills:

- handlers call `sdk.complete(...)` and get a token stream back; logging is invisible
- it is **impossible** to make an LLM call through the app without a `LogEvent` firing
- channel full → drop with a `tracing::warn`, never block the chat path
- previews pass through `sdk/redact.rs` **before** persistence: regex rules turn emails / phones / card numbers / key-shaped strings into `[REDACTED:*]`, and only then is the preview capped at 500 chars — redact-then-cap so a hard boundary can never slice a sensitive token in half (cost + privacy tradeoff vs full payload capture; unit-tested rules)

### Why a channel + worker instead of Kafka

Same shape: durable queue + consumer. Choosing in-process tokio mpsc:
- the chat path never blocks on DB regardless of DB health
- zero infra to run the demo
- honest tradeoff (documented): crash loses in-flight events. The fix is swapping the transport to Kafka/Redis Streams — the worker + producer signatures don't change.

### Multi-provider

`trait LlmProvider` (`provider_name`, `model_name`, `complete_stream(Vec<ChatMessage>) -> LlmStream`). Concrete impls: `OpenAIProvider`, `GeminiProvider`. Selection is one env var (`LLM_PROVIDER`), resolved once in `main.rs` and injected as `Arc<dyn LlmProvider>`. The SDK is written against the trait — it doesn't know which provider it wraps.

Streaming is first-class in the trait (`StreamChunk::Token / Done { usage }`), so SSE is a pass-through: Gemini bytes → provider pump → SDK pump → router pump → browser.

---

## Schema design

```sql
conversations   id uuid PK, session_id uuid, title text, created_at, updated_at
messages        id uuid PK, conversation_id FK (ON DELETE CASCADE), role, content, created_at
inference_logs  id uuid PK, conversation_id FK, message_id FK,
                model, provider, latency_ms, input_tokens, output_tokens,
                status ('success'|'error'), error_msg, input_preview, output_preview, created_at
```

Decisions and why:

- **`session_id` instead of `users`** — the assignment has no auth; a browser-local UUID keeps conversations private-enough for the demo while the schema stays honest about identity isolation. In production: `users` table + JWT, same FK shape.
- **UUIDs everywhere**, not serials — no cross-service coordination when IDs are pre-generated (see below), and enumeration is harder.
- **`inference_logs.message_id` links one assistant reply ↔ one inference call.** Logs always reference the assistant message ID, never the user's.
- **Placeholder write, then UPDATE** — the worker may beat the HTTP handler to its insert (both complete the same call ~0ms apart). The chat handler therefore inserts the assistant row **first** (empty content, pre-generated ID), then UPDATEs it when the stream ends. FK is always satisfiable; failed calls leave an empty assistant row + error log — a faithful record of "a reply was attempted."
- **status as TEXT + CHECK** — `success|error` with extensible values beats a boolean for "was this marked bad later" and aggregation queries stay simple (`FILTER (WHERE status='error')`); enum type is the upgrade path under a multi-write environment.
- **Indexes** on every lookup/filter column actually used (session_id, conversation_id, created_at, status) — measured from the dashboard's query patterns, not created defensively.
- **`updated_at` on conversations** drives the feed ordering; `LIMIT 20` history keeps OpenAI/Gemini context (and cost) bounded — the "short conversational context" requirement.

---

## Tradeoffs made

| Choice | I gave up | Because |
|---|---|---|
| tokio mpsc (in-mem) | durability across crashes | demo power:Kafka-free infra, 3-day flight |
| single sequential worker | ingest throughput | below dashboard-visible latency until ~20 rps bursts |
| placeholders for assistant rows | transient empty rows | guaranteed FK integrity under handler/worker races |
| previews capped 500 chars | full payload forensics | storage + PII surface control |
| no conversation-cancel endpoint | bonus checkbox | UI cancellation works via browser `fetch.abort()`; server-side cancellation is a `tokio::select!` with `watch::channel` threaded through pumps — clean but not worth the day |
| adapter-node, no NGINX | static CDN host | one service per container, front+back on distinct ports, CORS open for demo |
| multi-provider via env at boot | per-request provider pick | dashboard compares providers -> per-request switching makes the latency buckets un-comparable for the demo |

---

## Failure handling assumptions

- **LLM API errors** (401/429/503/404) are first-class telemetry — the SDK logs them with status=`error`, the handler returns the provider message, the browser renders it; dashboard error-rate lights up. Verified live: OpenAI over-quota (429) and model-not-found (404) both appear as structured rows.
- **Channel overflow** → drop + warn, chat unaffected. Bounded at 10k events.
- **Worker insert failure** → error logged, worker loops; no panic takes down the inserter.
- **Browser disconnect mid-stream** → pump's `send().await` errors, task exits; placeholder row remains with partial content + the log row — survives reload as honest history.
- **DB down at startup** → process refuses to start (pool connect + migrate must succeed). If DB dies mid-run, handlers get 5xx from sqlx; the worker just keeps failing per event.
- **Graceful shutdown** → ctrl-c drops the channel sender, worker drains remaining events, logs "ingestion worker stopped".

## Scaling notes

- Worker parallelism: N `tokio::spawn` consumers on the same `Receiver` — mpsc dispatches each item to exactly one, safe to scale.
- Batched inserts (`INSERT ... VALUES x100`) behind the same worker interface — biggest free win next.
- Swap mpsc for Redis Streams / Kafka when a second service (e.g. eval pipeline) must consume the same events; also enables replay.
- Postgres: the dashboard queries the raw logs table; at volume this moves to a materialized view / rollup table or ClickHouse. Read path behind a replica keeps writes hot.
- Latency percentiles via `percentile_cont` paid per query — move to t-digest rollups when the summary endpoint is called by many dashboards concurrently.

## With more time

- context-aware PII detection (NER model) — the shipped `redact.rs` covers emails/phones/cards/key shapes by regex
- conversation cancel endpoint + `watch::channel` cancellation through pumps
- prompt template/version columns on logs (replay a conversation against a different provider)
- provider-per-request routing in `AppState` (provider registry) + dashboard split
- t-digest latency rollups; OpenTelemetry spans end-to-end instead of hand-rolled fields

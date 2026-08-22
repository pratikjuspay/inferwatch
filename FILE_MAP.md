# File map — who talks to whom

Three views: dependency graph (code-level), runtime sequence (one chat message), boot order.

---

## 1. Dependency graph (who imports whom)

```mermaid
graph TD
    subgraph frontend
        UI["+page.svelte<br/>dashboard / conversations"]
        API["lib/api.ts"]
        STORES["lib/stores.ts"]
        UI --> API
        UI --> STORES
    end

    subgraph backend
        MAIN["main.rs<br/>(boot + wiring)"]
        STATE["state.rs<br/>AppState"]
        ERR["errors.rs<br/>AppError"]

        subgraph routes/
            CHAT["chat.rs"]
            CONV["conversations.rs"]
            METR["metrics.rs"]
        end

        subgraph db/
            DBC["conversations.rs"]
            DBM["messages.rs"]
            DBL["inference_logs.rs"]
        end

        subgraph providers/
            PMOD["mod.rs<br/>(trait LlmProvider)"]
            GEM["gemini.rs"]
            OAI["openai.rs"]
        end

        SDK["sdk/mod.rs<br/>InstrumentedProvider + LogEvent"]
        WORK["ingestion/worker.rs"]

        MAIN --> STATE
        MAIN --> SDK
        MAIN --> providers/
        MAIN --> routes/
        MAIN --> WORK

        STATE --> SDK
        CHAT -->|uses| STATE
        CHAT -->|uses| ERR
        CHAT -->|uses| DBC
        CHAT -->|uses| DBM
        CHAT -.->|"ChatMessage, StreamChunk"| PMOD
        CONV -->|uses| STATE & ERR & DBC & DBM
        METR -->|uses| STATE & ERR

        SDK --> PMOD
        PMOD --> GEM & OAI
        WORK -.->|"consumes LogEvent"| SDK
        WORK --> DBL
        DBL -.->|"reads LogEvent shape"| SDK
    end

    UI -.->|"HTTP + SSE"| CHAT
    UI -.->|"fetch"| CONV & METR

    classDef hot fill:#818cf8,stroke:#333,color:#fff
    class MAIN,CHAT,SDK,WORK hot
```

---

## 2. One chat message, end to end (sequence)

```mermaid
sequenceDiagram
    participant B as Browser
    participant CH as routes/chat.rs
    participant SDK as sdk/mod.rs
    participant PR as providers/gemini.rs
    participant G as Gemini API
    participant BUS as channel (log_tx/log_rx)
    participant W as ingestion/worker.rs
    participant DB as Postgres

    B->>CH: POST /api/chat/:id {session_id, message}
    Note over CH,DB: ── user path (blocks user) ──
    CH->>DB: SELECT conversation (ownership check)
    CH->>DB: set title (first message only)
    CH->>DB: INSERT user message row
    CH->>DB: touch conversation (updated_at)
    CH->>DB: SELECT last 20 messages
    Note over CH,DB: history EXCLUDES the placeholder —
    Note over CH,DB: it is inserted AFTER this SELECT on purpose
    CH->>DB: INSERT assistant placeholder row (empty content)
    CH->>SDK: sdk.complete(conv_id, msg_id, messages)
    SDK->>PR: complete_stream(messages)
    PR->>G: POST :streamGenerateContent?alt=sse
    loop per frame
        G-->>PR: SSE frames (tokens)
        PR-->>SDK: StreamChunk::Token …
        SDK-->>CH: forwarded chunks
        CH-->>B: SSE data: {type:"token", …}
    end
    G-->>PR: final frame (usageMetadata)
    PR-->>SDK: StreamChunk::Done{in,out}
    SDK->>BUS: try_send(LogEvent) — fires 1st
    SDK-->>CH: forwarded Done — browser gets it 2nd
    CH->>DB: UPDATE assistant row with full content
    CH-->>B: SSE data: {type:"done", …} then stream closes
    Note over CH,B: user has everything — user path ENDS
    Note over BUS,DB: ── bookkeeping path (async) ──
    BUS->>W: rx.recv() wakes worker
    W->>DB: INSERT inference_logs row
```

Three things strictly ordered now:

1. ownership check → title → user insert → touch → history → placeholder → SDK call
2. SDK fires LogEvent to the bus BEFORE forwarding "done" to the browser
3. assistant UPDATE (user path) happens BEFORE stream close; only `inference_logs` INSERT is off-path bookkeeping

**Error variant (same diagram, different branch):** provider returns Err (401/429/503/404) → SDK fires ONE LogEvent with `status="error"` + stored `error_msg` + partial output_preview → forwards error to browser as `{type:"error"}` SSE event → worker writes the row anyway. Errors are telemetry, not just failures.

---

## 3. Boot order (main.rs, top to bottom)

```mermaid
flowchart LR
    A["tokio runtime + env + logger"] --> B["PgPool connect<br/>(10 conns)"]
    B --> C["sqlx::migrate!<br/>3 tables + idx"]
    C --> D["mpsc channel<br/>cap 10_000"]
    D --> E["spawn worker<br/>(bg task, loop forever)"]
    E --> F["pick provider by env<br/>Arc dyn -> openai|gemini"]
    F --> G["InstrumentedProvider<br/>(provider, log_tx)"]
    G --> H["AppState { pool, sdk }"]
    H --> I["Router + cors + trace"]
    I --> J["axum::serve :3001"]
```

Any of steps B–F fails → process refuses to boot (no half-up server).

---

## 4. One-line per file (quick reference)

| File | Plain-English job | Touches |
|---|---|---|
| `main.rs` | assembly line: connects everything | everything |
| `state.rs` | the bag handed to every handler (pool + sdk) | sdk |
| `errors.rs` | converts errors → HTTP status + JSON | axum only |
| `providers/mod.rs` | neutral language: ChatMessage, StreamChunk, trait | futures, serde |
| `providers/gemini.rs` | knows how Google speaks (roles, SSE frames, usage) | reqwest |
| `providers/openai.rs` | knows how OpenAI speaks | reqwest |
| `sdk/mod.rs` | accountant: wraps calls, fires ONE LogEvent per call | providers/, channel |
| `ingestion/worker.rs` | mailbox collector: drains channel → one INSERT per event | db/, channel |
| `db/conversations.rs` | SQL for conversations table | sqlx |
| `db/messages.rs` | SQL for messages (incl. pre-generated IDs) | sqlx |
| `db/inference_logs.rs` | SQL for logs — worker-only | sqlx |
| `routes/chat.rs` | the orchestrator of one turn (verify→save→history→call→stream) | state, db, sdk |
| `routes/conversations.rs` | CRUD for conversations | state, db |
| `routes/metrics.rs` | 3 read-only aggregate SELECTs | state |
| `frontend/lib/api.ts` | typed fetch client + manual SSE parser | backend |
| `frontend/lib/stores.ts` | session UUID in localStorage | browser |
| `frontend/+page.svelte` | chat UI + composer + markdown + autoscroll | api.ts |
| `frontend/conversations/+page.svelte` | list + resume | api.ts |
| `frontend/dashboard/+page.svelte` | cards + chart + table, 5s poll | api.ts |

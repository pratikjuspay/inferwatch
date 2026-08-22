CREATE TABLE IF NOT EXISTS inference_logs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    message_id      UUID REFERENCES messages(id) ON DELETE SET NULL,
    model           TEXT NOT NULL,
    provider        TEXT NOT NULL,
    latency_ms      BIGINT NOT NULL,
    input_tokens    INTEGER,
    output_tokens   INTEGER,
    status          TEXT NOT NULL CHECK (status IN ('success', 'error')),
    error_msg       TEXT,
    input_preview   TEXT,
    output_preview  TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_inference_logs_conversation_id ON inference_logs(conversation_id);
CREATE INDEX IF NOT EXISTS idx_inference_logs_created_at ON inference_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_inference_logs_status ON inference_logs(status);

use crate::db::inference_logs;
use crate::sdk::LogEvent;
use sqlx::PgPool;
use tokio::sync::mpsc::Receiver;

/// Background task — runs forever alongside the HTTP server.
/// Drains LogEvents from the channel and writes them to Postgres.
/// Chat requests never wait for this; if the worker falls behind,
/// the channel buffer absorbs the burst.
pub async fn run(mut rx: Receiver<LogEvent>, pool: PgPool) {
    tracing::info!("ingestion worker started");
    while let Some(event) = rx.recv().await {
        if let Err(e) = inference_logs::insert(&pool, &event).await {
            // Never crash the worker on a bad write — log and continue.
            tracing::error!(error = %e, "failed to insert inference log");
        }
    }
    tracing::info!("ingestion worker stopped (channel closed)");
}

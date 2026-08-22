mod db;
mod errors;
mod ingestion;
mod providers;
mod routes;
mod sdk;
mod state;

use axum::{
    Router,
    routing::{get, post},
};
use sdk::{InstrumentedProvider, LogEvent};
use state::AppState;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,backend=debug".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")?;

    // 1. database pool
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;
    tracing::info!("connected to postgres");

    // 2. run migrations at startup — the app owns its schema
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("migrations applied");

    // 3. the event bus: SDK producers on one side, worker consumer on the other
    let (log_tx, log_rx) = tokio::sync::mpsc::channel::<LogEvent>(10_000);

    // 4. ingestion worker — lives outside the request path
    {
        let worker_pool = pool.clone();
        tokio::spawn(async move {
            ingestion::worker::run(log_rx, worker_pool).await;
        });
    }

    // 5. provider selection — the trait makes this a one-line swap
    let provider_kind = std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "gemini".into());
    let provider: Arc<dyn providers::LlmProvider> = match provider_kind.as_str() {
        "openai" => {
            let key = std::env::var("OPENAI_API_KEY")
                .expect("OPENAI_API_KEY required when LLM_PROVIDER=openai");
            let model =
                std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
            Arc::new(providers::openai::OpenAIProvider::new(key, model))
        }
        _ => {
            let key = std::env::var("GEMINI_API_KEY")
                .expect("GEMINI_API_KEY required when LLM_PROVIDER=gemini");
            let model = std::env::var("GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-3.1-flash-lite".into());
            Arc::new(providers::gemini::GeminiProvider::new(key, model))
        }
    };
    tracing::info!(
        provider = provider.provider_name(),
        model = provider.model_name(),
        "LLM provider selected"
    );
    let sdk = Arc::new(InstrumentedProvider::new(provider, log_tx));

    let state = AppState { pool, sdk };

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/conversations", post(routes::conversations::create_conversation))
        .route("/api/conversations", get(routes::conversations::list_conversations))
        .route(
            "/api/conversations/:id",
            get(routes::conversations::get_conversation),
        )
        .route("/api/chat/:conversation_id", post(routes::chat::chat))
        .route("/api/metrics/summary", get(routes::metrics::metrics_summary))
        .route("/api/metrics/latency", get(routes::metrics::latency_series))
        .route("/api/logs", get(routes::metrics::recent_logs))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    tracing::info!("listening on http://0.0.0.0:3001");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

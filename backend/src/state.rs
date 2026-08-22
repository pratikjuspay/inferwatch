use crate::sdk::InstrumentedProvider;
use sqlx::PgPool;
use std::sync::Arc;

/// Shared state injected into every handler via State(state).
/// Axum clones this per-request chain — Arc makes clones cheap.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub sdk: Arc<InstrumentedProvider>,
}

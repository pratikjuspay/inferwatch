use crate::errors::AppError;
use crate::state::AppState;
use axum::{Json, extract::State};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct MetricsSummary {
    pub total_calls: i64,
    pub error_count: i64,
    pub error_rate: f64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: Option<f64>,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub calls_last_hour: i64,
}

#[derive(Serialize)]
pub struct LatencyPoint {
    pub bucket: DateTime<Utc>,
    pub avg_latency_ms: f64,
    pub calls: i64,
    pub error_count: i64,
}

#[derive(Serialize)]
pub struct LogRowDto {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub model: String,
    pub provider: String,
    pub latency_ms: i64,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub status: String,
    pub error_msg: Option<String>,
    pub input_preview: Option<String>,
    pub output_preview: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Headline numbers for the dashboard.
pub async fn metrics_summary(
    State(state): State<AppState>,
) -> Result<Json<MetricsSummary>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(*)                                              AS "total_calls!: i64",
            COUNT(*) FILTER (WHERE status = 'error')              AS "error_count!: i64",
            COALESCE(AVG(latency_ms)::float8, 0)                  AS "avg_latency_ms!: f64",
            COALESCE(SUM(input_tokens), 0)                        AS "total_input_tokens!: i64",
            COALESCE(SUM(output_tokens), 0)                       AS "total_output_tokens!: i64",
            COUNT(*) FILTER (WHERE created_at > NOW() - INTERVAL '1 hour')
                                                                  AS "calls_last_hour!: i64",
            percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms)
                                                                  AS p95_latency_ms
        FROM inference_logs
        "#
    )
    .fetch_one(&state.pool)
    .await?;

    let error_rate = if row.total_calls > 0 {
        row.error_count as f64 / row.total_calls as f64
    } else {
        0.0
    };

    Ok(Json(MetricsSummary {
        total_calls: row.total_calls,
        error_count: row.error_count,
        error_rate,
        avg_latency_ms: row.avg_latency_ms,
        p95_latency_ms: row.p95_latency_ms,
        total_input_tokens: row.total_input_tokens,
        total_output_tokens: row.total_output_tokens,
        calls_last_hour: row.calls_last_hour,
    }))
}

/// Latency grouped into 5-minute buckets — powers the dashboard chart.
pub async fn latency_series(
    State(state): State<AppState>,
) -> Result<Json<Vec<LatencyPoint>>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            date_trunc('minute', created_at)
                - make_interval(mins => (EXTRACT(minute FROM created_at)::int % 5)) AS "bucket!",
            AVG(latency_ms)::float8  AS "avg_latency_ms!: f64",
            COUNT(*)         AS "calls!: i64",
            COUNT(*) FILTER (WHERE status = 'error') AS "error_count!: i64"
        FROM inference_logs
        WHERE created_at > NOW() - INTERVAL '24 hours'
        GROUP BY 1
        ORDER BY 1
        "#
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|r| LatencyPoint {
                bucket: r.bucket,
                avg_latency_ms: r.avg_latency_ms,
                calls: r.calls,
                error_count: r.error_count,
            })
            .collect(),
    ))
}

/// Most recent logs — the dashboard's raw table.
pub async fn recent_logs(
    State(state): State<AppState>,
) -> Result<Json<Vec<LogRowDto>>, AppError> {
    let rows = sqlx::query_as!(
        LogRowDto,
        r#"
        SELECT id, conversation_id, model, provider, latency_ms,
               input_tokens, output_tokens, status, error_msg,
               input_preview, output_preview, created_at
        FROM inference_logs
        ORDER BY created_at DESC
        LIMIT 100
        "#
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}

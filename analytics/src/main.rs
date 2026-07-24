//! Analytics API Server
//!
//! Provides REST endpoints for revenue predictions and analytics

use actix_web::{web, App, Error, HttpResponse, HttpServer};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use sqlx::Row;
use std::sync::Arc;
use tracing::{error, info};

mod predictor;
mod telemetry;
use predictor::{HistoricalStreamData, RevenuePrediction, RevenuePredictor};

/// Application state
pub struct AppState {
    pub db_pool: PgPool,
    pub predictor: RevenuePredictor,
}

/// Request parameters for revenue prediction
#[derive(Debug, Deserialize)]
pub struct PredictionRequest {
    pub creator_id: String,
    #[serde(default = "default_include_factors")]
    pub include_factors: bool,
}

fn default_include_factors() -> bool {
    true
}

/// Response structure for predictions
#[derive(Debug, Serialize)]
pub struct PredictionResponse {
    pub creator_id: String,
    pub predictions: Vec<RevenuePrediction>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Fetch historical stream data for a creator
async fn fetch_creator_history(
    pool: &PgPool,
    creator_id: &str,
    days: i32,
) -> Result<Vec<HistoricalStreamData>, sqlx::Error> {
    let query = r#"
        SELECT 
            timestamp,
            revenue,
            active_streams,
            cancellations
        FROM creator_analytics
        WHERE creator_id = $1
          AND timestamp >= NOW() - INTERVAL '1 day' * $2
        ORDER BY timestamp ASC
    "#;

    let rows = sqlx::query_as::<_, HistoricalStreamData>(query)
        .bind(creator_id)
        .bind(days)
        .fetch_all(pool)
        .await?;

    Ok(rows)
}

/// GET /health - Health check endpoint
async fn health_check() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}

/// POST /api/v1/predict/revenue - Generate revenue predictions
async fn predict_revenue(
    data: web::Data<Arc<AppState>>,
    req: web::Json<PredictionRequest>,
) -> Result<HttpResponse, Error> {
    info!(
        service.name = env!("CARGO_PKG_NAME"),
        service.version = env!("CARGO_PKG_VERSION"),
        enduser.id = %req.creator_id,
        http.request.method = "POST",
        url.path = "/api/v1/predict/revenue",
        "generating revenue prediction"
    );

    // Fetch 90 days of historical data
    match fetch_creator_history(&data.db_pool, &req.creator_id, 90).await {
        Ok(history) => {
            if history.len() < 10 {
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Insufficient historical data. Need at least 10 data points."
                })));
            }

            let predictions = data.predictor.generate_all_predictions(&history);

            Ok(HttpResponse::Ok().json(PredictionResponse {
                creator_id: req.creator_id.clone(),
                predictions,
                generated_at: chrono::Utc::now(),
            }))
        }
        Err(e) => {
            error!("Failed to fetch creator history: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch historical data"
            })))
        }
    }
}

/// GET /api/v1/analytics/{creator_id}/streams - Get current stream statistics
async fn get_stream_stats(
    data: web::Data<Arc<AppState>>,
    path: web::Path<String>,
) -> Result<HttpResponse, Error> {
    let creator_id = path.into_inner();

    let query = r#"
        SELECT 
            COUNT(*) as total_streams,
            SUM(monthly_value) as total_mrr,
            AVG(monthly_value) as avg_stream_value,
            COUNT(CASE WHEN status = 'active' THEN 1 END) as active_count,
            COUNT(CASE WHEN status = 'cancelled' THEN 1 END) as cancelled_count
        FROM revenue_streams
        WHERE creator_id = $1
    "#;

    match sqlx::query(query)
        .bind(&creator_id)
        .fetch_optional(&data.db_pool)
        .await
    {
        Ok(Some(row)) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "creator_id": creator_id,
            "total_streams": row.get::<i64, _>("total_streams"),
            "total_mrr": row.get::<f64, _>("total_mrr").unwrap_or(0.0),
            "avg_stream_value": row.get::<f64, _>("avg_stream_value").unwrap_or(0.0),
            "active_streams": row.get::<i64, _>("active_count").unwrap_or(0),
            "churned_streams": row.get::<i64, _>("cancelled_count").unwrap_or(0),
        }))),
        Ok(None) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "Creator not found"
        }))),
        Err(e) => {
            error!("Failed to fetch stream stats: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to fetch statistics"
            })))
        }
    }
}

/// Initialize and run the analytics server
pub async fn run_server(db_pool: PgPool) -> std::io::Result<()> {
    let app_state = Arc::new(AppState {
        db_pool,
        predictor: RevenuePredictor::new(),
    });

    let log_resource = telemetry::init(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    info!(
        service.name = log_resource.service_name,
        service.version = log_resource.service_version,
        deployment.environment.name = %log_resource.deployment_environment,
        network.local.address = "0.0.0.0",
        network.local.port = 8080_u16,
        url.scheme = "http",
        "starting analytics api server"
    );

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .route("/health", web::get().to(health_check))
            .route("/api/v1/predict/revenue", web::post().to(predict_revenue))
            .route(
                "/api/v1/analytics/{creator_id}/streams",
                web::get().to(get_stream_stats),
            )
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}

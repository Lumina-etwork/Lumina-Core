//! Social Backend - Comment System & Messaging
//! 
//! Main entry point for the social features API server

use actix_web::{web, App, HttpServer, middleware};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use tracing::{error, info};

mod comments;
mod messaging;
mod websocket;
mod telemetry;

/// Application state
pub struct AppState {
    pub db_pool: PgPool,
}

/// Health check response
#[derive(serde::Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Health check endpoint
async fn health_check() -> actix_web::Result<web::Json<HealthResponse>> {
    Ok(web::Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}

/// Authentication middleware (simplified - use JWT in production)
async fn authenticate(
    req: actix_web::HttpRequest,
) -> Result<uuid::Uuid, actix_web::Error> {
    // In production, validate JWT token from Authorization header
    // For now, extract user_id from X-User-ID header (for testing)
    let user_id_str = req.headers()
        .get("X-User-ID")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("Missing user ID"))?;

    uuid::Uuid::parse_str(user_id_str)
        .map_err(|_| actix_web::error::ErrorUnauthorized("Invalid user ID format"))
}

/// Initialize and run the social API server
pub async fn run_server(db_pool: PgPool) -> std::io::Result<()> {
    let app_state = Arc::new(AppState { db_pool });

    let log_resource = telemetry::init(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    info!(
        service.name = log_resource.service_name,
        service.version = log_resource.service_version,
        deployment.environment.name = %log_resource.deployment_environment,
        network.local.address = "0.0.0.0",
        network.local.port = 8081_u16,
        url.scheme = "http",
        "starting social api server"
    );

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            // Health check
            .route("/health", web::get().to(health_check))
            
            // Comment endpoints
            .service(
                web::scope("/api/v1/comments")
                    .route("", web::post().to(comments::create_comment))
                    .route("/{creator_id}", web::get().to(comments::get_comments))
                    .route("/{comment_id}", web::put().to(comments::update_comment))
                    .route("/{comment_id}", web::delete().to(comments::delete_comment))
                    .route("/{comment_id}/like", web::post().to(comments::like_comment))
            )
            
            // Messaging endpoints
            .service(
                web::scope("/api/v1/messages")
                    .route("", web::post().to(messaging::send_message))
                    .route("/conversations", web::get().to(messaging::get_conversations))
                    .route("/{recipient_id}", web::get().to(messaging::get_messages))
                    .route("/{message_id}", web::delete().to(messaging::delete_message))
                    .route("/{message_id}/read", web::put().to(messaging::mark_message_as_read))
            )
            
            // WebSocket endpoint for real-time messaging
            .route("/ws", web::get().to(websocket::ws_route))
    })
    .bind("0.0.0.0:8081")?
    .run()
    .await
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost:5432/lumina".to_string());
    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    run_server(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_rt::test]
    async fn test_health_check_response() {
        let response = actix_web::test::call_service(
            &App::new().route("/health", web::get().to(health_check)),
            actix_web::test::TestRequest::get().uri("/health").to_request(),
        ).await;
        
        assert!(response.status().is_success());
    }
}

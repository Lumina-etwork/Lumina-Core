//! Comment System API
//!
//! Implements exclusive threaded comments gated for fans with active subscriptions

use actix_web::{web, Error, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use uuid::Uuid;
use validator::Validate;

/// Threaded comment structure
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Comment {
    pub id: Uuid,
    pub creator_id: Uuid,
    pub fan_id: Uuid,
    pub parent_comment_id: Option<Uuid>,
    pub content: String,
    pub is_edited: bool,
    pub is_deleted: bool,
    pub like_count: i32,
    pub created_at: DateTime<Utc>,
    pub replies: Vec<Comment>, // Nested replies
}

/// Request to create a comment
#[derive(Debug, Validate, Deserialize)]
pub struct CreateCommentRequest {
    pub creator_id: Uuid,
    pub content: String,
    #[validate(length(min = 1, max = 5000))]
    pub parent_comment_id: Option<Uuid>,
}

/// Request to update a comment
#[derive(Debug, Validate, Deserialize)]
pub struct UpdateCommentRequest {
    #[validate(length(min = 1, max = 5000))]
    pub content: String,
}

/// Comment list response (paginated)
#[derive(Debug, Serialize)]
pub struct CommentListResponse {
    pub comments: Vec<Comment>,
    pub total_count: i64,
    pub page: i64,
    pub per_page: i64,
    pub has_more: bool,
}

/// Verify fan has active subscription to creator
async fn verify_subscription(
    pool: &PgPool,
    fan_id: Uuid,
    creator_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let exists = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM subscriptions
            WHERE fan_id = $1
              AND creator_id = $2
              AND status = 'active'
        )
        "#,
    )
    .bind(fan_id)
    .bind(creator_id)
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

/// POST /api/v1/comments - Create a new comment
pub async fn create_comment(
    pool: web::Data<PgPool>,
    fan_id: web::ReqData<Uuid>,
    req: web::Json<CreateCommentRequest>,
) -> Result<HttpResponse, Error> {
    // Validate request
    if let Err(e) = req.validate() {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Validation failed",
            "details": e.to_string()
        })));
    }

    // Verify fan has active subscription
    let has_subscription = verify_subscription(&pool, *fan_id, req.creator_id)
        .await
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
        })?;

    if !has_subscription {
        return Ok(HttpResponse::Forbidden().json(serde_json::json!({
            "error": "Active subscription required to comment"
        })));
    }

    // Create comment
    let comment = sqlx::query_as::<_, Comment>(
        r#"
        INSERT INTO comments (creator_id, fan_id, parent_comment_id, content)
        VALUES ($1, $2, $3, $4)
        RETURNING id, creator_id, fan_id, parent_comment_id, content, 
                  is_edited, is_deleted, like_count, created_at
        "#,
    )
    .bind(req.creator_id)
    .bind(*fan_id)
    .bind(req.parent_comment_id)
    .bind(&req.content)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to create comment: {}", e))
    })?;

    Ok(HttpResponse::Created().json(comment))
}

/// GET /api/v1/comments/{creator_id} - Get all comments for a creator (threaded)
pub async fn get_comments(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, Error> {
    let creator_id = path.into_inner();
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20);
    let offset = (page - 1) * per_page;

    // Get top-level comments
    let comments = sqlx::query_as::<_, Comment>(
        r#"
        SELECT c.id, c.creator_id, c.fan_id, c.parent_comment_id, c.content,
               c.is_edited, c.is_deleted, c.like_count, c.created_at
        FROM comments c
        WHERE c.creator_id = $1 AND c.parent_comment_id IS NULL
        ORDER BY c.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(creator_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to fetch comments: {}", e))
    })?;

    // Get total count
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM comments WHERE creator_id = $1 AND parent_comment_id IS NULL",
    )
    .bind(creator_id)
    .fetch_one(pool.get_ref())
    .await
    .unwrap_or(0);

    // Fetch nested replies for each comment
    let mut threaded_comments = Vec::new();
    for mut comment in comments {
        let replies = fetch_replies(&pool, comment.id).await?;
        comment.replies = replies;
        threaded_comments.push(comment);
    }

    Ok(HttpResponse::Ok().json(CommentListResponse {
        comments: threaded_comments,
        total_count: total,
        page,
        per_page,
        has_more: offset + per_page < total,
    }))
}

/// Helper function to fetch nested replies
async fn fetch_replies(pool: &PgPool, parent_id: Uuid) -> Result<Vec<Comment>, Error> {
    let replies = sqlx::query_as::<_, Comment>(
        r#"
        SELECT c.id, c.creator_id, c.fan_id, c.parent_comment_id, c.content,
               c.is_edited, c.is_deleted, c.like_count, c.created_at
        FROM comments c
        WHERE c.parent_comment_id = $1
        ORDER BY c.created_at ASC
        "#,
    )
    .bind(parent_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to fetch replies: {}", e))
    })?;

    Ok(replies)
}

/// PUT /api/v1/comments/{comment_id} - Update a comment
pub async fn update_comment(
    pool: web::Data<PgPool>,
    fan_id: web::ReqData<Uuid>,
    path: web::Path<Uuid>,
    req: web::Json<UpdateCommentRequest>,
) -> Result<HttpResponse, Error> {
    let comment_id = path.into_inner();

    // Verify ownership
    let is_owner = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM comments WHERE id = $1 AND fan_id = $2)",
    )
    .bind(comment_id)
    .bind(*fan_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Database error: {}", e)))?;

    if !is_owner {
        return Ok(HttpResponse::Forbidden().json(serde_json::json!({
            "error": "Not authorized to edit this comment"
        })));
    }

    // Update comment
    let comment = sqlx::query_as::<_, Comment>(
        r#"
        UPDATE comments
        SET content = $1, is_edited = TRUE, updated_at = NOW()
        WHERE id = $2
        RETURNING id, creator_id, fan_id, parent_comment_id, content,
                  is_edited, is_deleted, like_count, created_at
        "#,
    )
    .bind(&req.content)
    .bind(comment_id)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to update comment: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(comment))
}

/// DELETE /api/v1/comments/{comment_id} - Delete a comment (soft delete)
pub async fn delete_comment(
    pool: web::Data<PgPool>,
    fan_id: web::ReqData<Uuid>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, Error> {
    let comment_id = path.into_inner();

    // Soft delete
    sqlx::query(
        "UPDATE comments SET is_deleted = TRUE, content = '[deleted]' WHERE id = $1 AND fan_id = $2",
    )
    .bind(comment_id)
    .bind(*fan_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to delete comment: {}", e))
    })?;

    Ok(HttpResponse::NoContent().finish())
}

/// POST /api/v1/comments/{comment_id}/like - Like a comment
pub async fn like_comment(
    pool: web::Data<PgPool>,
    fan_id: web::ReqData<Uuid>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, Error> {
    let comment_id = path.into_inner();

    // Insert like (ignore if already exists)
    sqlx::query(
        "INSERT INTO comment_likes (comment_id, fan_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(comment_id)
    .bind(*fan_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to like comment: {}", e))
    })?;

    // Update like count
    let like_count: i32 = sqlx::query_scalar("SELECT like_count FROM comments WHERE id = $1")
        .bind(comment_id)
        .fetch_one(pool.get_ref())
        .await
        .map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Failed to get like count: {}", e))
        })?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "comment_id": comment_id,
        "like_count": like_count + 1
    })))
}

/// Pagination parameters
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}

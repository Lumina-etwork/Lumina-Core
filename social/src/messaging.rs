//! End-to-End Encrypted Messaging System
//!
//! Implements E2E encryption using ChaCha20-Poly1305 for direct messages

use actix_web::{web, Error, HttpResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use uuid::Uuid;

// Encryption types
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;

/// Encrypted message structure
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub recipient_id: Uuid,
    pub encrypted_content: String, // Base64 encoded
    pub nonce: String,             // Base64 encoded
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub sent_at: DateTime<Utc>,
}

/// Request to send a message
#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub recipient_id: Uuid,
    pub encrypted_content: String, // Already encrypted client-side
    pub nonce: String,
}

/// Conversation summary
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Conversation {
    pub id: Uuid,
    pub participant_id: Uuid, // Other participant
    pub participant_name: String,
    pub last_message_at: Option<DateTime<Utc>>,
    pub last_message_preview: Option<String>,
    pub unread_count: i64,
}

/// Verify user has permission to DM (Gold tier or higher)
async fn verify_messaging_permission(
    pool: &PgPool,
    fan_id: Uuid,
    creator_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let has_permission = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM subscriptions s
            JOIN subscription_tiers st ON s.tier_id = st.id
            WHERE s.fan_id = $1
              AND s.creator_id = $2
              AND s.status = 'active'
              AND st.tier_level >= 3
        )
        "#,
    )
    .bind(fan_id)
    .bind(creator_id)
    .fetch_one(pool)
    .await?;

    Ok(has_permission)
}

/// POST /api/v1/messages - Send an encrypted message
pub async fn send_message(
    pool: web::Data<PgPool>,
    sender_id: web::ReqData<Uuid>,
    req: web::Json<SendMessageRequest>,
) -> Result<HttpResponse, Error> {
    // For fan-to-creator messages, verify tier permission
    let is_creator_recipient =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM creators WHERE user_id = $1)")
            .bind(req.recipient_id)
            .fetch_one(pool.get_ref())
            .await
            .map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
            })?;

    if is_creator_recipient {
        // Find which creator this fan is trying to message
        let creator_id = req.recipient_id;

        let has_permission = verify_messaging_permission(&pool, *sender_id, creator_id)
            .await
            .map_err(|e| {
                actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
            })?;

        if !has_permission {
            return Ok(HttpResponse::Forbidden().json(serde_json::json!({
                "error": "Gold tier subscription required to message this creator"
            })));
        }
    }

    // Create message
    let message = sqlx::query_as::<_, Message>(
        r#"
        INSERT INTO messages (sender_id, recipient_id, encrypted_content, nonce)
        VALUES ($1, $2, $3, $4)
        RETURNING id, sender_id, recipient_id, encrypted_content, nonce,
                  is_read, read_at, sent_at
        "#,
    )
    .bind(*sender_id)
    .bind(req.recipient_id)
    .bind(&req.encrypted_content)
    .bind(&req.nonce)
    .fetch_one(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to send message: {}", e))
    })?;

    // Update conversation metadata
    update_conversation(
        &pool,
        *sender_id,
        req.recipient_id,
        &message.encrypted_content,
    )
    .await?;

    Ok(HttpResponse::Created().json(message))
}

/// GET /api/v1/messages/conversations - Get user's conversations
pub async fn get_conversations(
    pool: web::Data<PgPool>,
    user_id: web::ReqData<Uuid>,
) -> Result<HttpResponse, Error> {
    let conversations = sqlx::query_as::<_, Conversation>(
        r#"
        SELECT 
            c.id,
            CASE 
                WHEN c.participant_1 = $1 THEN c.participant_2
                ELSE c.participant_1
            END as participant_id,
            u.username as participant_name,
            c.last_message_at,
            c.last_message_preview,
            c.unread_count
        FROM conversations c
        JOIN users u ON (
            CASE 
                WHEN c.participant_1 = $1 THEN c.participant_2 = u.id
                ELSE c.participant_1 = u.id
            END
        )
        WHERE c.participant_1 = $1 OR c.participant_2 = $1
        ORDER BY c.last_message_at DESC NULLS LAST
        "#,
    )
    .bind(*user_id)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to fetch conversations: {}", e))
    })?;

    Ok(HttpResponse::Ok().json(conversations))
}

/// GET /api/v1/messages/{recipient_id} - Get message history with a user
pub async fn get_messages(
    pool: web::Data<PgPool>,
    user_id: web::ReqData<Uuid>,
    path: web::Path<Uuid>,
    query: web::Query<crate::comments::PaginationParams>,
) -> Result<HttpResponse, Error> {
    let other_user_id = path.into_inner();
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(50);
    let offset = (page - 1) * per_page;

    let messages = sqlx::query_as::<_, Message>(
        r#"
        SELECT id, sender_id, recipient_id, encrypted_content, nonce,
               is_read, read_at, sent_at
        FROM messages
        WHERE (sender_id = $1 AND recipient_id = $2)
           OR (sender_id = $2 AND recipient_id = $1)
          AND deleted_by_sender = FALSE
          AND deleted_by_recipient = FALSE
        ORDER BY sent_at DESC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(*user_id)
    .bind(other_user_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to fetch messages: {}", e))
    })?;

    // Mark received messages as read
    sqlx::query(
        r#"
        UPDATE messages
        SET is_read = TRUE, read_at = NOW()
        WHERE sender_id = $1 AND recipient_id = $2 AND is_read = FALSE
        "#,
    )
    .bind(other_user_id)
    .bind(*user_id)
    .execute(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(messages))
}

/// Helper function to update conversation metadata
async fn update_conversation(
    pool: &PgPool,
    sender_id: Uuid,
    recipient_id: Uuid,
    encrypted_content: &str,
) -> Result<(), sqlx::Error> {
    // Decode preview (first 50 chars of decrypted content would be ideal,
    // but we'll use a placeholder since server can't decrypt)
    let preview = "[Encrypted Message]";

    sqlx::query(
        r#"
        INSERT INTO conversations (participant_1, participant_2, last_message_at, last_message_preview)
        VALUES ($1, $2, NOW(), $3)
        ON CONFLICT (participant_1, participant_2) DO UPDATE
        SET last_message_at = NOW(),
            last_message_preview = EXCLUDED.last_message_preview
        "#,
    )
    .bind(sender_id)
    .bind(recipient_id)
    .bind(preview)
    .execute(pool)
    .await?;

    // Increment unread count for recipient
    sqlx::query(
        r#"
        UPDATE conversations
        SET unread_count = unread_count + 1
        WHERE (participant_1 = $1 AND participant_2 = $2)
           OR (participant_1 = $2 AND participant_2 = $1)
        "#,
    )
    .bind(sender_id)
    .bind(recipient_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// DELETE /api/v1/messages/{message_id} - Delete a message
pub async fn delete_message(
    pool: web::Data<PgPool>,
    user_id: web::ReqData<Uuid>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, Error> {
    let message_id = path.into_inner();

    // Mark as deleted by user
    sqlx::query(
        r#"
        UPDATE messages
        SET deleted_by_sender = TRUE
        WHERE id = $1 AND sender_id = $2
        "#,
    )
    .bind(message_id)
    .bind(*user_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to delete message: {}", e))
    })?;

    Ok(HttpResponse::NoContent().finish())
}

/// PUT /api/v1/messages/{message_id}/read - Mark message as read
pub async fn mark_message_as_read(
    pool: web::Data<PgPool>,
    recipient_id: web::ReqData<Uuid>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, Error> {
    let message_id = path.into_inner();

    sqlx::query(
        "UPDATE messages SET is_read = TRUE, read_at = NOW() WHERE id = $1 AND recipient_id = $2",
    )
    .bind(message_id)
    .bind(*recipient_id)
    .execute(pool.get_ref())
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Failed to mark as read: {}", e))
    })?;

    Ok(HttpResponse::Ok().finish())
}

/// Utility: Encrypt message content (client should do this, but providing utility)
pub fn encrypt_message(
    content: &[u8],
    key: &[u8],
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)?;

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(
        nonce,
        Payload {
            msg: content,
            aad: &[],
        },
    )?;

    Ok((base64::encode(&ciphertext), base64::encode(&nonce_bytes)))
}

/// Utility: Decrypt message content (client should do this)
pub fn decrypt_message(
    encrypted: &[u8],
    nonce: &[u8],
    key: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let cipher = ChaCha20Poly1305::new_from_slice(key)?;
    let nonce = Nonce::from_slice(nonce);

    let plaintext = cipher.decrypt(
        nonce,
        Payload {
            msg: encrypted,
            aad: &[],
        },
    )?;

    Ok(plaintext)
}

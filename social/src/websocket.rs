//! WebSocket Server for Real-time Messaging
//!
//! Implements WebSocket connections for instant message delivery

use actix::{prelude::*, Actor, StreamHandler};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);

/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

/// Message types for WebSocket communication
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    /// Send a new message to a recipient
    SendMessage {
        recipient_id: Uuid,
        encrypted_content: String,
        nonce: String,
    },
    /// Mark messages as read
    MarkRead { message_ids: Vec<Uuid> },
    /// Typing indicator
    Typing {
        conversation_id: Uuid,
        is_typing: bool,
    },
    /// Server acknowledgment
    Ack {
        message_id: Option<Uuid>,
        status: String,
    },
    /// Error message
    Error { message: String },
}

/// WebSocket session for each user
pub struct WsSession {
    /// Unique session id
    id: Uuid,
    /// Client must send ping at least once per this duration
    hb: Instant,
    /// User ID associated with this session
    user_id: Uuid,
    /// Database pool reference (via Addr pattern in production)
    db_available: bool,
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb = Instant::now();

        // Start heartbeat
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // Check client still responds
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                warn!(
                    service.name = env!("CARGO_PKG_NAME"),
                    service.version = env!("CARGO_PKG_VERSION"),
                    session.id = %act.id,
                    enduser.id = %act.user_id,
                    "websocket client heartbeat failed; disconnecting"
                );
                ctx.stop();
                return;
            }

            // Send ping
            ctx.ping(b"");
        });

        info!(
            service.name = env!("CARGO_PKG_NAME"),
            service.version = env!("CARGO_PKG_VERSION"),
            session.id = %self.id,
            enduser.id = %self.user_id,
            db.available = self.db_available,
            "websocket session started"
        );
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        info!(
            service.name = env!("CARGO_PKG_NAME"),
            service.version = env!("CARGO_PKG_VERSION"),
            session.id = %self.id,
            enduser.id = %self.user_id,
            "websocket session stopped"
        );
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                // Handle text message (JSON)
                match serde_json::from_str::<WsMessage>(&text) {
                    Ok(ws_msg) => self.handle_ws_message(ws_msg, ctx),
                    Err(e) => {
                        let error_msg = WsMessage::Error {
                            message: format!("Invalid message format: {}", e),
                        };
                        ctx.text(serde_json::to_string(&error_msg).unwrap());
                    }
                }
            }
            Ok(ws::Message::Binary(bin)) => {
                // Could support binary messages for efficiency
                debug!(
                    service.name = env!("CARGO_PKG_NAME"),
                    service.version = env!("CARGO_PKG_VERSION"),
                    session.id = %self.id,
                    enduser.id = %self.user_id,
                    message.size = bin.len(),
                    "received websocket binary message"
                );
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
        }
    }
}

impl WsSession {
    pub fn new(user_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            hb: Instant::now(),
            user_id,
            db_available: false,
        }
    }

    /// Handle incoming WebSocket message
    fn handle_ws_message(&mut self, msg: WsMessage, ctx: &mut ws::WebsocketContext<Self>) {
        match msg {
            WsMessage::SendMessage {
                recipient_id,
                encrypted_content,
                nonce,
            } => {
                // In production, save to database and forward to recipient
                info!(
                    service.name = env!("CARGO_PKG_NAME"),
                    service.version = env!("CARGO_PKG_VERSION"),
                    session.id = %self.id,
                    enduser.id = %self.user_id,
                    messaging.destination.id = %recipient_id,
                    messaging.message.body.size = encrypted_content.len(),
                    messaging.message.envelope.size = nonce.len(),
                    "websocket send message requested"
                );

                // Send acknowledgment
                let ack = WsMessage::Ack {
                    message_id: Some(Uuid::new_v4()),
                    status: "sent".to_string(),
                };
                ctx.text(serde_json::to_string(&ack).unwrap());

                // TODO: Forward to recipient's WebSocket session if online
                // This would require an Actor address registry
            }
            WsMessage::MarkRead { message_ids } => {
                info!(
                    service.name = env!("CARGO_PKG_NAME"),
                    service.version = env!("CARGO_PKG_VERSION"),
                    session.id = %self.id,
                    enduser.id = %self.user_id,
                    messaging.message.count = message_ids.len(),
                    "websocket messages marked read"
                );

                // Update database and notify sender
                let ack = WsMessage::Ack {
                    message_id: None,
                    status: "read_receipt_sent".to_string(),
                };
                ctx.text(serde_json::to_string(&ack).unwrap());
            }
            WsMessage::Typing {
                conversation_id,
                is_typing,
            } => {
                // Forward typing indicator to conversation participant
                debug!(
                    service.name = env!("CARGO_PKG_NAME"),
                    service.version = env!("CARGO_PKG_VERSION"),
                    session.id = %self.id,
                    enduser.id = %self.user_id,
                    messaging.conversation.id = %conversation_id,
                    messaging.typing = is_typing,
                    "websocket typing indicator received"
                );

                // TODO: Forward to other participant
            }
            _ => {
                debug!(
                    service.name = env!("CARGO_PKG_NAME"),
                    service.version = env!("CARGO_PKG_VERSION"),
                    session.id = %self.id,
                    enduser.id = %self.user_id,
                    "unhandled websocket message type"
                );
            }
        }
    }
}

/// Upgrade HTTP request to WebSocket connection
pub async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    query: web::Query<WsQueryParams>,
) -> Result<HttpResponse, Error> {
    // Extract user ID from query params (in production, use JWT token)
    let user_id = Uuid::parse_str(&query.user_id)
        .map_err(|_| actix_web::error::ErrorBadRequest("Invalid user_id format"))?;

    // Create WebSocket session
    let mut session = WsSession::new(user_id);

    // Start WebSocket responder
    ws::start(&mut session, &req, stream)
}

/// Query parameters for WebSocket connection
#[derive(Debug, Deserialize)]
pub struct WsQueryParams {
    /// User ID for authentication
    pub user_id: String,
    /// Optional: JWT token for production auth
    pub token: Option<String>,
}

/// Message broadcaster (for sending to multiple users)
pub struct MessageBroadcaster {
    /// Active sessions: user_id -> session address
    sessions: std::collections::HashMap<Uuid, Addr<WsSession>>,
}

impl MessageBroadcaster {
    pub fn new() -> Self {
        Self {
            sessions: std::collections::HashMap::new(),
        }
    }

    /// Register a session
    pub fn register(&mut self, user_id: Uuid, addr: Addr<WsSession>) {
        self.sessions.insert(user_id, addr);
        info!(enduser.id = %user_id, websocket.sessions = self.sessions.len(), "websocket user connected");
    }

    /// Unregister a session
    pub fn unregister(&mut self, user_id: Uuid) {
        self.sessions.remove(&user_id);
        info!(enduser.id = %user_id, websocket.sessions = self.sessions.len(), "websocket user disconnected");
    }

    /// Send message to specific user
    pub fn send_to_user(&self, recipient_id: Uuid, message: WsMessage) {
        if let Some(addr) = self.sessions.get(&recipient_id) {
            // Convert to text and send via WebSocket
            if let Ok(text) = serde_json::to_string(&message) {
                addr.do_send(ws::Message::Text(text.into()));
            }
        } else {
            warn!(messaging.destination.id = %recipient_id, "websocket recipient offline; message not delivered");
        }
    }

    /// Broadcast message to all connected users
    pub fn broadcast(&self, message: WsMessage) {
        if let Ok(text) = serde_json::to_string(&message) {
            for (_, addr) in &self.sessions {
                addr.do_send(ws::Message::Text(text.clone().into()));
            }
        }
    }
}

impl Default for MessageBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

use actix::{Actor, ActorContext, AsyncContext, Handler, Message, StreamHandler};
/// WebSocket server for real-time notifications
///
/// This module provides:
/// - WebSocket handler using actix-web
/// - Connection management with heartbeat
/// - Broadcast of whale alerts to all connected clients
/// - Reconnection handling
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::sync::atomic::{AtomicUsize, Ordering};

use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

/// Global connection counter
static CONNECTION_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Get current number of WebSocket connections
pub fn connection_count() -> usize {
    CONNECTION_COUNT.load(Ordering::SeqCst)
}

/// WebSocket session actor
pub struct WsSession {
    /// Unique session id
    id: usize,
    /// Client must send ping at least once per CLIENT_TIMEOUT
    heartbeat: Instant,
    /// Broadcast receiver for notifications
    ws_receiver: Option<broadcast::Receiver<String>>,
}

impl WsSession {
    pub fn new(ws_receiver: broadcast::Receiver<String>) -> Self {
        let id = CONNECTION_COUNT.fetch_add(1, Ordering::SeqCst);
        info!(session_id = id, "WebSocket client connected");
        Self {
            id,
            heartbeat: Instant::now(),
            ws_receiver: Some(ws_receiver),
        }
    }

    /// Start the heartbeat process
    fn start_heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.heartbeat) > CLIENT_TIMEOUT {
                warn!(
                    session_id = act.id,
                    "WebSocket client heartbeat timeout, disconnecting"
                );
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }

    /// Start listening for broadcast messages
    fn start_broadcast_listener(&mut self, ctx: &mut ws::WebsocketContext<Self>) {
        if let Some(mut receiver) = self.ws_receiver.take() {
            let addr = ctx.address();
            actix_rt::spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok(msg) => {
                            addr.do_send(BroadcastMessage(msg));
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(lagged = n, "WebSocket broadcast lagged");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            });
        }
    }
}

/// Internal message for broadcast delivery
#[derive(Message)]
#[rtype(result = "()")]
struct BroadcastMessage(String);

impl Handler<BroadcastMessage> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: BroadcastMessage, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.text(msg.0);
    }
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.start_heartbeat(ctx);
        self.start_broadcast_listener(ctx);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        CONNECTION_COUNT.fetch_sub(1, Ordering::SeqCst);
        info!(session_id = self.id, "WebSocket client disconnected");
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.heartbeat = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                debug!(session_id = self.id, text = %text, "Received text from client");
                // Echo back or handle commands
                ctx.text(format!("{{\"type\":\"ack\",\"received\":\"{}\"}}", text));
            }
            Ok(ws::Message::Binary(_)) => {
                debug!(
                    session_id = self.id,
                    "Received binary from client (ignored)"
                );
            }
            Ok(ws::Message::Close(reason)) => {
                debug!(session_id = self.id, reason = ?reason, "WebSocket close received");
                ctx.close(reason);
                ctx.stop();
            }
            Err(e) => {
                error!(session_id = self.id, error = %e, "WebSocket protocol error");
                ctx.stop();
            }
            _ => {}
        }
    }
}

/// WebSocket upgrade handler
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    ws_sender: web::Data<broadcast::Sender<String>>,
) -> Result<HttpResponse, actix_web::Error> {
    let receiver = ws_sender.subscribe();
    let session = WsSession::new(receiver);
    ws::start(session, &req, stream)
}

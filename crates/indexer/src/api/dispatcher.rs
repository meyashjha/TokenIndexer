/// Alert Dispatcher - processes whale purchase events and broadcasts to WebSocket clients
///
/// This component:
/// - Subscribes to WhalePurchaseEvent from the message queue
/// - Formats notification payloads
/// - Sends formatted notifications to the WebSocket broadcast channel
use crate::queue::{Event, MessageQueue};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, watch};
use tracing::{debug, error, info, warn};

/// Notification payload sent to WebSocket clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketNotification {
    #[serde(rename = "type")]
    pub notification_type: String,
    pub data: serde_json::Value,
    pub timestamp: String,
}

/// Alert Dispatcher component
pub struct AlertDispatcher {
    queue: Arc<dyn MessageQueue>,
    ws_sender: broadcast::Sender<String>,
}

impl AlertDispatcher {
    /// Create a new Alert Dispatcher
    pub fn new(queue: Arc<dyn MessageQueue>, ws_sender: broadcast::Sender<String>) -> Self {
        Self { queue, ws_sender }
    }

    /// Start the dispatcher loop
    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) -> Result<()> {
        info!("Alert Dispatcher starting");

        let mut stream = self.queue.subscribe().await?;

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Alert Dispatcher received shutdown signal");
                        break;
                    }
                }
                event = stream.next() => {
                    match event {
                        Ok(Some(event)) => {
                            self.handle_event(event).await;
                        }
                        Ok(None) => {
                            debug!("Event stream ended");
                            break;
                        }
                        Err(e) => {
                            warn!(error = %e, "Error receiving event");
                        }
                    }
                }
            }
        }

        info!("Alert Dispatcher stopped");
        Ok(())
    }

    /// Handle a single event
    async fn handle_event(&self, event: Event) {
        let notification = match &event {
            Event::WhalePurchase(purchase_event) => {
                let data =
                    serde_json::to_value(&purchase_event.alert).unwrap_or(serde_json::Value::Null);

                WebSocketNotification {
                    notification_type: "whale_purchase".to_string(),
                    data,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                }
            }
            Event::WhaleIdentified(whale_event) => {
                let data =
                    serde_json::to_value(&whale_event.wallet).unwrap_or(serde_json::Value::Null);

                WebSocketNotification {
                    notification_type: "whale_identified".to_string(),
                    data,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                }
            }
            Event::TokenCreated(token_event) => {
                let data =
                    serde_json::to_value(&token_event.token).unwrap_or(serde_json::Value::Null);

                WebSocketNotification {
                    notification_type: "token_created".to_string(),
                    data,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                }
            }
        };

        // Serialize to JSON for WebSocket broadcast
        match serde_json::to_string(&notification) {
            Ok(json) => {
                let receivers = self.ws_sender.send(json).unwrap_or(0);
                debug!(
                    notification_type = %notification.notification_type,
                    receivers = receivers,
                    "Notification broadcast"
                );
            }
            Err(e) => {
                error!(error = %e, "Failed to serialize notification");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use crate::queue::InMemoryQueue;

    #[tokio::test]
    async fn test_dispatcher_formats_whale_purchase() {
        let queue = Arc::new(InMemoryQueue::new());
        let (ws_sender, mut ws_receiver) = broadcast::channel(100);
        let dispatcher = AlertDispatcher::new(queue.clone(), ws_sender);

        let alert = WhaleAlert::new(
            "TestWallet".to_string(),
            "TestToken".to_string(),
            100.0,
            WalletMetrics {
                win_rate: 0.85,
                average_roi: 4.5,
                wallet_type: WalletType::EstablishedSniper,
            },
        );

        let event = Event::WhalePurchase(crate::queue::WhalePurchaseEvent {
            alert: alert.clone(),
        });

        dispatcher.handle_event(event).await;

        let received = ws_receiver.recv().await.unwrap();
        let notification: WebSocketNotification = serde_json::from_str(&received).unwrap();
        assert_eq!(notification.notification_type, "whale_purchase");
        assert!(notification.data.get("wallet_address").is_some());
    }
}

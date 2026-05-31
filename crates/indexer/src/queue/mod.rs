#![allow(unused_doc_comments)]

/// Message queue for event processing
///
/// This module provides:
/// - Event type definitions
/// - Message queue trait
/// - In-memory queue implementation using tokio broadcast channels
/// - Event buffering with configurable limits
use crate::models::{TokenMetadata, WhaleAlert, WhaleWallet};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, info, warn};

/// Default buffer size for event buffering
const DEFAULT_BUFFER_CAPACITY: usize = 10_000;

/// Default broadcast channel capacity
const DEFAULT_CHANNEL_CAPACITY: usize = 1_000;

/// Event types for the message queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    TokenCreated(TokenCreatedEvent),
    WhaleIdentified(WhaleIdentifiedEvent),
    WhalePurchase(WhalePurchaseEvent),
}

/// Event published when a new token is discovered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCreatedEvent {
    pub token: TokenMetadata,
}

/// Event published when a whale wallet is identified
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhaleIdentifiedEvent {
    pub wallet: WhaleWallet,
}

/// Event published when a whale makes a purchase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhalePurchaseEvent {
    pub alert: WhaleAlert,
}

/// Message queue trait for event publishing and subscribing
#[async_trait]
pub trait MessageQueue: Send + Sync {
    /// Publish an event to the queue
    async fn publish(&self, event: Event) -> Result<()>;

    /// Subscribe to events from the queue
    async fn subscribe(&self) -> Result<Box<dyn EventStream>>;

    /// Get the number of events published
    fn event_count(&self) -> usize;
}

/// Event stream for consuming events
#[async_trait]
pub trait EventStream: Send {
    /// Receive the next event
    async fn next(&mut self) -> Result<Option<Event>>;
}

/// In-memory message queue implementation using tokio broadcast channels
pub struct InMemoryQueue {
    sender: broadcast::Sender<Event>,
    event_count: AtomicUsize,
    buffer: Arc<Mutex<Vec<Event>>>,
    buffer_capacity: usize,
    buffer_overflow_count: AtomicUsize,
}

impl InMemoryQueue {
    /// Create a new in-memory queue with default settings
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CHANNEL_CAPACITY, DEFAULT_BUFFER_CAPACITY)
    }

    /// Create a new in-memory queue with custom capacity
    pub fn with_capacity(channel_capacity: usize, buffer_capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(channel_capacity);
        info!(
            channel_capacity = channel_capacity,
            buffer_capacity = buffer_capacity,
            "In-memory message queue created"
        );
        Self {
            sender,
            event_count: AtomicUsize::new(0),
            buffer: Arc::new(Mutex::new(Vec::with_capacity(buffer_capacity))),
            buffer_capacity,
            buffer_overflow_count: AtomicUsize::new(0),
        }
    }

    /// Get the number of overflowed events (dropped due to buffer limit)
    pub fn overflow_count(&self) -> usize {
        self.buffer_overflow_count.load(Ordering::SeqCst)
    }

    /// Get the current buffer size
    pub async fn buffer_size(&self) -> usize {
        self.buffer.lock().await.len()
    }

    /// Drain buffered events (useful when queue comes back online)
    pub async fn drain_buffer(&self) -> Vec<Event> {
        let mut buffer = self.buffer.lock().await;
        std::mem::take(&mut *buffer)
    }
}

impl Default for InMemoryQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageQueue for InMemoryQueue {
    async fn publish(&self, event: Event) -> Result<()> {
        let count = self.event_count.fetch_add(1, Ordering::SeqCst) + 1;

        match self.sender.send(event.clone()) {
            Ok(receivers) => {
                debug!(
                    event_count = count,
                    receivers = receivers,
                    "Event published to queue"
                );
                Ok(())
            }
            Err(_) => {
                // No subscribers - buffer the event
                let mut buffer = self.buffer.lock().await;
                if buffer.len() >= self.buffer_capacity {
                    // Buffer full - drop oldest
                    self.buffer_overflow_count.fetch_add(1, Ordering::SeqCst);
                    warn!(
                        buffer_size = buffer.len(),
                        buffer_capacity = self.buffer_capacity,
                        "Event buffer full, dropping oldest event"
                    );
                    buffer.remove(0);
                }
                buffer.push(event);
                debug!(
                    buffer_size = buffer.len(),
                    "Event buffered (no subscribers)"
                );
                Ok(())
            }
        }
    }

    async fn subscribe(&self) -> Result<Box<dyn EventStream>> {
        let receiver = self.sender.subscribe();
        Ok(Box::new(BroadcastEventStream { receiver }))
    }

    fn event_count(&self) -> usize {
        self.event_count.load(Ordering::SeqCst)
    }
}

/// Event stream backed by a tokio broadcast receiver
struct BroadcastEventStream {
    receiver: broadcast::Receiver<Event>,
}

#[async_trait]
impl EventStream for BroadcastEventStream {
    async fn next(&mut self) -> Result<Option<Event>> {
        match self.receiver.recv().await {
            Ok(event) => Ok(Some(event)),
            Err(broadcast::error::RecvError::Closed) => Ok(None),
            Err(broadcast::error::RecvError::Lagged(count)) => {
                warn!(
                    lagged_events = count,
                    "Event stream lagged, some events lost"
                );
                // Try to receive the next available event
                match self.receiver.recv().await {
                    Ok(event) => Ok(Some(event)),
                    Err(_) => Ok(None),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use chrono::Utc;

    fn make_token_event() -> Event {
        Event::TokenCreated(TokenCreatedEvent {
            token: TokenMetadata::new(
                "TestMint123".to_string(),
                Utc::now(),
                LaunchpadSource::PumpFun,
                12345,
            ),
        })
    }

    fn make_whale_event() -> Event {
        Event::WhaleIdentified(WhaleIdentifiedEvent {
            wallet: WhaleWallet::new(
                "TestWallet123".to_string(),
                0.85,
                4.5,
                WalletType::EstablishedSniper,
                50,
            ),
        })
    }

    #[tokio::test]
    async fn test_publish_and_subscribe() {
        let queue = InMemoryQueue::new();
        let mut stream = queue.subscribe().await.unwrap();

        let event = make_token_event();
        queue.publish(event).await.unwrap();

        let received = tokio::time::timeout(std::time::Duration::from_millis(100), stream.next())
            .await
            .unwrap()
            .unwrap();

        assert!(received.is_some());
        match received.unwrap() {
            Event::TokenCreated(e) => assert_eq!(e.token.mint_address, "TestMint123"),
            _ => panic!("Expected TokenCreated event"),
        }
    }

    #[tokio::test]
    async fn test_event_count() {
        let queue = InMemoryQueue::new();
        assert_eq!(queue.event_count(), 0);

        queue.publish(make_token_event()).await.unwrap();
        assert_eq!(queue.event_count(), 1);

        queue.publish(make_whale_event()).await.unwrap();
        assert_eq!(queue.event_count(), 2);
    }

    #[tokio::test]
    async fn test_buffering_without_subscribers() {
        let queue = InMemoryQueue::with_capacity(10, 5);

        // Publish without subscribers - should buffer
        queue.publish(make_token_event()).await.unwrap();
        queue.publish(make_whale_event()).await.unwrap();

        assert_eq!(queue.buffer_size().await, 2);

        // Drain buffer
        let buffered = queue.drain_buffer().await;
        assert_eq!(buffered.len(), 2);
        assert_eq!(queue.buffer_size().await, 0);
    }

    #[tokio::test]
    async fn test_buffer_overflow() {
        let queue = InMemoryQueue::with_capacity(10, 3);

        // Fill buffer beyond capacity
        for _ in 0..5 {
            queue.publish(make_token_event()).await.unwrap();
        }

        assert_eq!(queue.buffer_size().await, 3); // Capped at capacity
        assert_eq!(queue.overflow_count(), 2); // 2 events dropped
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let queue = InMemoryQueue::new();
        let mut stream1 = queue.subscribe().await.unwrap();
        let mut stream2 = queue.subscribe().await.unwrap();

        queue.publish(make_token_event()).await.unwrap();

        let r1 = tokio::time::timeout(std::time::Duration::from_millis(100), stream1.next())
            .await
            .unwrap()
            .unwrap();

        let r2 = tokio::time::timeout(std::time::Duration::from_millis(100), stream2.next())
            .await
            .unwrap()
            .unwrap();

        assert!(r1.is_some());
        assert!(r2.is_some());
    }
}

/// Property-based tests for event publishing and buffering
#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::models::*;
    use chrono::Utc;
    use proptest::prelude::*;

    fn make_event(idx: usize) -> Event {
        Event::TokenCreated(TokenCreatedEvent {
            token: TokenMetadata::new(
                format!("Mint{}", idx),
                Utc::now(),
                LaunchpadSource::PumpFun,
                idx as u64,
            ),
        })
    }

    /// **Property 3: Event Publishing on Storage**
    /// For any storage operation, successfully storing the data SHALL publish
    /// exactly one corresponding event to the Message_Queue.
    /// **Validates: Requirements 1.5, 2.9, 3.7**
    proptest! {
        #[test]
        fn prop_event_count_matches_publishes(
            num_events in 1usize..=50,
        ) {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let queue = InMemoryQueue::new();
                let mut _stream = queue.subscribe().await.unwrap();

                for i in 0..num_events {
                    queue.publish(make_event(i)).await.unwrap();
                }

                prop_assert_eq!(queue.event_count(), num_events);
                Ok(())
            })?;
        }
    }

    /// **Property 20: Event Buffering with Limit**
    /// The system SHALL buffer events in memory up to the configured limit,
    /// and SHALL reject or drop events that exceed the limit.
    /// **Validates: Requirements 9.3**
    proptest! {
        #[test]
        fn prop_buffer_respects_limit(
            buffer_capacity in 5usize..=50,
            num_events in 10usize..=100,
        ) {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let queue = InMemoryQueue::with_capacity(10, buffer_capacity);
                // Don't subscribe - all events go to buffer

                for i in 0..num_events {
                    queue.publish(make_event(i)).await.unwrap();
                }

                let buffer_size = queue.buffer_size().await;
                prop_assert!(
                    buffer_size <= buffer_capacity,
                    "Buffer size {} exceeded capacity {}",
                    buffer_size, buffer_capacity
                );

                if num_events > buffer_capacity {
                    let expected_overflow = num_events - buffer_capacity;
                    prop_assert_eq!(
                        queue.overflow_count(),
                        expected_overflow,
                        "Expected {} overflows, got {}",
                        expected_overflow,
                        queue.overflow_count()
                    );
                }

                Ok(())
            })?;
        }
    }
}

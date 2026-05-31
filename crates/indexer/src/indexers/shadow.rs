#![allow(unused_doc_comments)]

/// The Shadow - Whale Tracker
///
/// Monitors proven whale wallets for real-time purchase detection
/// and generates alerts.
use crate::config::ShadowConfig;
use crate::database::{WhaleAlertRepo, WhaleWalletRepo};
use crate::models::*;
use crate::queue::{Event, MessageQueue, WhalePurchaseEvent};
use crate::rpc::parser;
use crate::rpc::RpcClientPool;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

/// The Shadow indexer component
pub struct Shadow {
    config: ShadowConfig,
    rpc_client: Arc<RpcClientPool>,
    whale_repo: Arc<dyn WhaleWalletRepo>,
    alert_repo: Arc<dyn WhaleAlertRepo>,
    queue: Arc<dyn MessageQueue>,
    /// Track last seen signature for each wallet to avoid duplicates
    last_signatures: tokio::sync::Mutex<HashMap<String, String>>,
}

impl Shadow {
    /// Create a new Shadow indexer
    pub fn new(
        config: ShadowConfig,
        rpc_client: Arc<RpcClientPool>,
        whale_repo: Arc<dyn WhaleWalletRepo>,
        alert_repo: Arc<dyn WhaleAlertRepo>,
        queue: Arc<dyn MessageQueue>,
    ) -> Self {
        Self {
            config,
            rpc_client,
            whale_repo,
            alert_repo,
            queue,
            last_signatures: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Start the Shadow polling loop
    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) -> Result<()> {
        info!(
            polling_interval = self.config.polling_interval_secs,
            max_wallets = self.config.max_concurrent_wallets,
            "Shadow starting polling loop"
        );

        let interval = std::time::Duration::from_secs(self.config.polling_interval_secs);

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Shadow received shutdown signal");
                        break;
                    }
                }
                _ = tokio::time::sleep(interval) => {
                    if let Err(e) = self.monitor_whales().await {
                        error!(error = %e, "Shadow monitoring error");
                    }
                }
            }
        }

        info!("Shadow stopped");
        Ok(())
    }

    /// Monitor all whale wallets for new purchases
    async fn monitor_whales(&self) -> Result<()> {
        let wallets = self.whale_repo.get_all().await?;

        if wallets.is_empty() {
            debug!("No whale wallets to monitor");
            return Ok(());
        }

        let wallet_count = wallets.len().min(self.config.max_concurrent_wallets);
        debug!(wallet_count = wallet_count, "Monitoring whale wallets");

        // Lock signatures map once to read previous values
        let last_sigs_lock = self.last_signatures.lock().await;

        // Process wallets concurrently using tokio tasks
        let mut handles = Vec::new();
        for wallet in wallets.into_iter().take(self.config.max_concurrent_wallets) {
            let rpc = self.rpc_client.clone();
            let alert_repo = self.alert_repo.clone();
            let queue = self.queue.clone();
            // Look up only the last signature for this specific wallet address
            let last_sig = last_sigs_lock.get(&wallet.wallet_address).cloned();

            handles.push(tokio::spawn(async move {
                Self::monitor_single_wallet(&wallet, &rpc, &alert_repo, &queue, last_sig).await
            }));
        }

        // Drop lock before waiting for async tasks to run to prevent blocking
        drop(last_sigs_lock);

        // Collect results
        for handle in handles {
            match handle.await {
                Ok(Ok(Some((addr, sig)))) => {
                    // Update last seen signature
                    self.last_signatures.lock().await.insert(addr, sig);
                }
                Ok(Ok(None)) => {}
                Ok(Err(e)) => {
                    warn!(error = %e, "Whale monitoring task error");
                }
                Err(e) => {
                    error!(error = %e, "Whale monitoring task panicked");
                }
            }
        }

        Ok(())
    }

    /// Monitor a single whale wallet for new purchases
    async fn monitor_single_wallet(
        wallet: &WhaleWallet,
        rpc: &RpcClientPool,
        alert_repo: &Arc<dyn WhaleAlertRepo>,
        queue: &Arc<dyn MessageQueue>,
        last_seen: Option<String>,
    ) -> Result<Option<(String, String)>> {
        // Fetch recent signatures for this wallet
        let signatures = rpc.get_signatures_for_address(&wallet.wallet_address, Some(5))?;

        if signatures.is_empty() {
            return Ok(None);
        }

        for sig_info in &signatures {
            // Skip if we've already seen this signature
            if let Some(ref last) = last_seen {
                if &sig_info.signature == last {
                    break;
                }
            }

            let signature = match sig_info.signature.parse() {
                Ok(sig) => sig,
                Err(_) => continue,
            };

            // Fetch transaction details
            let tx = match rpc.get_transaction(&signature) {
                Ok(tx) => tx,
                Err(e) => {
                    debug!(
                        wallet = %wallet.wallet_address,
                        signature = %sig_info.signature,
                        error = %e,
                        "Failed to fetch whale transaction"
                    );
                    continue;
                }
            };

            // Check if this is a token purchase
            if let Ok(Some(purchase)) = parser::extract_purchase_data(&tx) {
                // Create alert
                let alert = WhaleAlert::new(
                    wallet.wallet_address.clone(),
                    purchase.token_address.clone(),
                    purchase.amount,
                    wallet.metrics(),
                );

                // Store alert
                if let Err(e) = alert_repo.insert(&alert).await {
                    error!(
                        wallet = %wallet.wallet_address,
                        token = %purchase.token_address,
                        error = %e,
                        "Failed to store whale alert"
                    );
                    continue;
                }

                // Publish event
                if let Err(e) = queue
                    .publish(Event::WhalePurchase(WhalePurchaseEvent {
                        alert: alert.clone(),
                    }))
                    .await
                {
                    error!(
                        wallet = %wallet.wallet_address,
                        error = %e,
                        "Failed to publish whale purchase event"
                    );
                }

                info!(
                    wallet = %wallet.wallet_address,
                    token = %purchase.token_address,
                    amount = purchase.amount,
                    win_rate = wallet.win_rate,
                    roi = wallet.average_roi,
                    "Whale purchase detected!"
                );
            }
        }

        // Return updated signature if available
        if let Some(first) = signatures.first() {
            Ok(Some((wallet.wallet_address.clone(), first.signature.clone())))
        } else {
            Ok(None)
        }
    }
}

/// Property tests for alert payload structure
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// **Property 11: Alert Payload Structure**
    /// For any whale purchase event, the alert payload SHALL contain:
    /// wallet address, token address, purchase amount, timestamp, and wallet metrics.
    ///
    /// **Validates: Requirements 3.8, 4.2**
    proptest! {
        #[test]
        fn prop_alert_payload_has_required_fields(
            wallet_addr in "[a-zA-Z0-9]{32,44}",
            token_addr in "[a-zA-Z0-9]{32,44}",
            amount in 0.001f64..=10000.0,
            win_rate in 0.0f64..=1.0,
            avg_roi in 0.0f64..=100.0,
        ) {
            let metrics = WalletMetrics {
                win_rate,
                average_roi: avg_roi,
                wallet_type: WalletType::EstablishedSniper,
            };

            let alert = WhaleAlert::new(
                wallet_addr.clone(),
                token_addr.clone(),
                amount,
                metrics.clone(),
            );

            // Verify all required fields are present
            prop_assert_eq!(&alert.wallet_address, &wallet_addr);
            prop_assert_eq!(&alert.token_address, &token_addr);
            prop_assert_eq!(alert.purchase_amount, amount);
            prop_assert!(!alert.id.is_nil(), "Alert ID should not be nil");
            prop_assert!(alert.timestamp <= chrono::Utc::now(), "Timestamp should not be in the future");

            // Verify wallet metrics
            prop_assert_eq!(alert.wallet_metrics.win_rate, win_rate);
            prop_assert_eq!(alert.wallet_metrics.average_roi, avg_roi);
            prop_assert_eq!(&alert.wallet_metrics.wallet_type, &WalletType::EstablishedSniper);

            // Verify JSON serialization preserves all fields
            let json = serde_json::to_value(&alert).unwrap();
            prop_assert!(json.get("id").is_some());
            prop_assert!(json.get("timestamp").is_some());
            prop_assert!(json.get("wallet_address").is_some());
            prop_assert!(json.get("token_address").is_some());
            prop_assert!(json.get("purchase_amount").is_some());
            prop_assert!(json.get("wallet_metrics").is_some());
        }
    }
}

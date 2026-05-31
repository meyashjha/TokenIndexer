/// The Scout - Launchpad Indexer
///
/// Polls Solana RPC nodes to detect new token creation events
/// on Pump.fun and Raydium launchpads.
use crate::config::ScoutConfig;
use crate::database::TokenRepo;
use crate::models::{LaunchpadSource, TokenMetadata};
use crate::queue::{Event, MessageQueue, TokenCreatedEvent};
use crate::rpc::parser;
use crate::rpc::RpcClientPool;
use anyhow::Result;
use chrono::Utc;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

/// The Scout indexer component
pub struct Scout {
    config: ScoutConfig,
    rpc_client: Arc<RpcClientPool>,
    token_repo: Arc<dyn TokenRepo>,
    queue: Arc<dyn MessageQueue>,
    /// Cache of already-seen signature strings with FIFO order to avoid duplicates
    seen_signatures: tokio::sync::Mutex<(HashSet<String>, VecDeque<String>)>,
    /// Timestamps of tokens indexed in the current sliding window
    indexed_tokens_timestamps: tokio::sync::Mutex<Vec<std::time::Instant>>,
}

impl Scout {
    /// Create a new Scout indexer
    pub fn new(
        config: ScoutConfig,
        rpc_client: Arc<RpcClientPool>,
        token_repo: Arc<dyn TokenRepo>,
        queue: Arc<dyn MessageQueue>,
    ) -> Self {
        Self {
            config,
            rpc_client,
            token_repo,
            queue,
            seen_signatures: tokio::sync::Mutex::new((HashSet::new(), VecDeque::new())),
            indexed_tokens_timestamps: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    /// Start the Scout polling loop
    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) -> Result<()> {
        info!(
            polling_interval = self.config.polling_interval_secs,
            pumpfun = %self.config.pumpfun_program_id,
            raydium = %self.config.raydium_program_id,
            "Scout starting polling loop"
        );

        let interval = std::time::Duration::from_secs(self.config.polling_interval_secs);

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Scout received shutdown signal");
                        break;
                    }
                }
                _ = tokio::time::sleep(interval) => {
                    if let Err(e) = self.poll_launchpads().await {
                        error!(error = %e, "Scout polling error");
                    }
                }
            }
        }

        info!("Scout stopped");
        Ok(())
    }

    /// Poll both launchpad programs for new token creation events
    async fn poll_launchpads(&self) -> Result<()> {
        // Check sliding window limit before polling in a scoped block to release the lock immediately
        {
            let now = std::time::Instant::now();
            let ten_minutes_ago = now - std::time::Duration::from_secs(600);
            let mut timestamps = self.indexed_tokens_timestamps.lock().await;
            timestamps.retain(|&t| t > ten_minutes_ago);
            if timestamps.len() >= 50 {
                debug!(
                    "Sliding window limit reached: already indexed 50 tokens in the last 10 minutes."
                );
                return Ok(());
            }
        }

        // Poll Pump.fun
        if let Err(e) = self
            .poll_program(&self.config.pumpfun_program_id, LaunchpadSource::PumpFun)
            .await
        {
            warn!(
                program = %self.config.pumpfun_program_id,
                error = %e,
                "Failed to poll Pump.fun"
            );
        }

        // Poll Raydium
        if let Err(e) = self
            .poll_program(&self.config.raydium_program_id, LaunchpadSource::Raydium)
            .await
        {
            warn!(
                program = %self.config.raydium_program_id,
                error = %e,
                "Failed to poll Raydium"
            );
        }

        Ok(())
    }

    /// Poll a specific launchpad program for new signatures
    async fn poll_program(&self, program_id: &str, source: LaunchpadSource) -> Result<()> {
        let signatures = self
            .rpc_client
            .get_signatures_for_address(program_id, Some(100))?;

        let mut lock = self.seen_signatures.lock().await;
        let (seen, order) = &mut *lock;

        for sig_info in signatures {
            let sig_str = sig_info.signature.clone();

            // Skip already-seen signatures
            if seen.contains(&sig_str) {
                continue;
            }

            // Parse signature
            let signature = match sig_str.parse() {
                Ok(sig) => sig,
                Err(e) => {
                    warn!(signature = %sig_str, error = %e, "Failed to parse signature");
                    continue;
                }
            };

            // Fetch transaction details
            let tx = match self.rpc_client.get_transaction(&signature) {
                Ok(tx) => tx,
                Err(e) => {
                    warn!(
                        signature = %sig_str,
                        error = %e,
                        "Failed to fetch transaction details"
                    );
                    // Do NOT mark as seen, so we can retry on subsequent polls
                    continue;
                }
            };

            // Mark as seen only after successful fetch
            seen.insert(sig_str.clone());
            order.push_back(sig_str.clone());

            // Parse token mint from transaction
            match parser::parse_token_mint(
                &tx,
                &self.config.pumpfun_program_id,
                &self.config.raydium_program_id,
            ) {
                Ok(Some(parsed_mint)) => {
                    // Check sliding window rate limit again inside the loop
                    let now = std::time::Instant::now();
                    let ten_minutes_ago = now - std::time::Duration::from_secs(600);
                    let mut timestamps = self.indexed_tokens_timestamps.lock().await;
                    timestamps.retain(|&t| t > ten_minutes_ago);
                    if timestamps.len() >= 50 {
                        debug!("Sliding window limit reached during processing: indexed 50 tokens in the last 10 minutes.");
                        break;
                    }

                    let token = TokenMetadata::new(
                        parsed_mint.mint_address.clone(),
                        Utc::now(),
                        source.clone(),
                        parsed_mint.slot,
                    );

                    // Store in database
                    if let Err(e) = self.token_repo.insert(&token).await {
                        error!(
                            mint = %parsed_mint.mint_address,
                            error = %e,
                            "Failed to store token"
                        );
                        continue;
                    }

                    timestamps.push(std::time::Instant::now());

                    // Publish event
                    if let Err(e) = self
                        .queue
                        .publish(Event::TokenCreated(TokenCreatedEvent {
                            token: token.clone(),
                        }))
                        .await
                    {
                        error!(
                            mint = %parsed_mint.mint_address,
                            error = %e,
                            "Failed to publish token created event"
                        );
                    }

                    info!(
                        mint = %token.mint_address,
                        source = %source,
                        slot = token.slot_number,
                        "New token discovered ({}/50 in last 10m)",
                        timestamps.len()
                    );
                }
                Ok(None) => {
                    debug!(signature = %sig_str, "No token mint found in transaction");
                }
                Err(e) => {
                    warn!(
                        signature = %sig_str,
                        error = %e,
                        "Failed to parse token mint from transaction"
                    );
                }
            }
        }

        // Limit the size of seen signatures using FIFO eviction to prevent unbounded memory growth
        while seen.len() > 10_000 {
            if let Some(oldest) = order.pop_front() {
                seen.remove(&oldest);
            } else {
                break;
            }
        }

        Ok(())
    }
}

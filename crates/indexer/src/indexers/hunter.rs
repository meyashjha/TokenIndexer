use crate::config::HunterConfig;
use crate::database::{EarlyBuyerRepo, TokenRepo, WhaleWalletRepo};
use crate::models::{EarlyBuyerTransaction, TokenMetadata, TokenTransaction};
use crate::queue::MessageQueue;
use crate::rpc::{parser, RpcClientPool};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use solana_sdk::signature::Signature;
use std::sync::Arc;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

pub struct Hunter {
    config: HunterConfig,
    rpc_client: Arc<RpcClientPool>,
    token_repo: Arc<dyn TokenRepo>,
    whale_repo: Arc<dyn WhaleWalletRepo>,
    early_buyer_repo: Arc<dyn EarlyBuyerRepo>,
    _queue: Arc<dyn MessageQueue>,
}

impl Hunter {
    pub fn new(
        config: HunterConfig,
        rpc_client: Arc<RpcClientPool>,
        token_repo: Arc<dyn TokenRepo>,
        whale_repo: Arc<dyn WhaleWalletRepo>,
        early_buyer_repo: Arc<dyn EarlyBuyerRepo>,
        queue: Arc<dyn MessageQueue>,
    ) -> Self {
        Self {
            config,
            rpc_client,
            token_repo,
            whale_repo,
            early_buyer_repo,
            _queue: queue,
        }
    }

    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) -> Result<()> {
        info!(
            polling_interval = self.config.polling_interval_secs,
            max_signatures = self.config.max_signatures_per_token,
            "Hunter starting polling loop"
        );

        let interval = std::time::Duration::from_secs(self.config.polling_interval_secs);

        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Hunter received shutdown signal");
                        break;
                    }
                }
                _ = tokio::time::sleep(interval) => {
                    if let Err(e) = self.poll_tokens().await {
                        error!(error = %e, "Hunter polling error");
                    }
                }
            }
        }

        info!("Hunter stopped");
        Ok(())
    }

    async fn poll_tokens(&self) -> Result<()> {
        let mut tokens = self.token_repo.list_active_tokens(7, 50).await?;

        if tokens.is_empty() {
            tokens = self
                .token_repo
                .list_unanalyzed_mature(self.config.token_maturity_minutes, 50)
                .await?;
        }

        if tokens.is_empty() {
            debug!("No tokens ready for Hunter indexing");
            return Ok(());
        }

        for token in tokens {
            if let Err(err) = self.index_token(&token).await {
                warn!(mint = %token.mint_address, error = %err, "Failed to index token");
            }
        }

        Ok(())
    }

    async fn index_token(&self, token: &TokenMetadata) -> Result<()> {
        let until = token
            .last_indexed_signature
            .as_deref()
            .and_then(|signature| signature.parse::<Signature>().ok());

        let signatures = self.rpc_client.get_signatures_for_address_until(
            &token.mint_address,
            Some(self.config.max_signatures_per_token),
            until,
        )?;

        let newest_signature = signatures.first().map(|sig| sig.signature.clone());

        let mut token_txs = Vec::new();
        let mut early_buyer_txs = Vec::new();

        for sig_info in signatures.iter().rev() {
            let signature = match sig_info.signature.parse::<Signature>() {
                Ok(signature) => signature,
                Err(err) => {
                    warn!(signature = %sig_info.signature, error = %err, "Invalid transaction signature");
                    continue;
                }
            };

            let tx = match self.rpc_client.get_transaction(&signature) {
                Ok(tx) => tx,
                Err(err) => {
                    debug!(signature = %sig_info.signature, error = %err, "Failed to fetch token transaction");
                    continue;
                }
            };

            let Some(purchase) = parser::extract_purchase_data(&tx)? else {
                continue;
            };

            if purchase.token_address != token.mint_address {
                continue;
            }

            let timestamp = sig_info
                .block_time
                .and_then(|unix| Utc.timestamp_opt(unix, 0).single())
                .unwrap_or_else(Utc::now);

            token_txs.push(TokenTransaction {
                signature: sig_info.signature.clone(),
                token_mint: token.mint_address.clone(),
                buyer_address: purchase.buyer_address.clone(),
                amount: purchase.amount,
                slot_number: purchase.slot,
                timestamp,
            });

            let slot_delta = purchase.slot as i64 - token.slot_number as i64;
            if (0..=self.config.early_buyer_window_secs as i64).contains(&slot_delta) {
                early_buyer_txs.push(EarlyBuyerTransaction {
                    signature: sig_info.signature.clone(),
                    wallet_address: purchase.buyer_address.clone(),
                    token_mint: token.mint_address.clone(),
                    purchase_slot: purchase.slot,
                    slot_delta,
                    amount: purchase.amount,
                    timestamp,
                });
            }

            let _ = self
                .whale_repo
                .get_by_address(&purchase.buyer_address)
                .await?;
        }

        self.early_buyer_repo
            .insert_token_transactions_batch(&token_txs)
            .await?;

        self.early_buyer_repo
            .insert_batch(&early_buyer_txs)
            .await?;

        self.token_repo
            .update_indexed_status(&token.mint_address, newest_signature.as_deref())
            .await?;

        if !token.analyzed {
            self.token_repo.mark_analyzed(&token.mint_address).await?;
        }

        info!(
            mint = %token.mint_address,
            newest_signature = ?newest_signature,
            "Hunter indexed token transactions"
        );

        Ok(())
    }
}

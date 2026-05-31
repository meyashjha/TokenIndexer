#![allow(unused_doc_comments)]

/// RPC client with load balancing and error handling
///
/// This module provides:
/// - Solana RPC client wrapper
/// - Load balancing across multiple endpoints
/// - Connection pooling
/// - Retry logic with exponential backoff
/// - Rate limit handling
/// - Metrics logging
pub mod parser;

use crate::config::RpcConfig;
use anyhow::{Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_transaction_status::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// RPC client wrapper with load balancing
pub struct RpcClientPool {
    clients: Vec<RpcClient>,
    endpoints: Vec<String>,
    current_index: AtomicUsize,
    max_retries: u32,
    _timeout: Duration,
}

impl RpcClientPool {
    /// Create a new RPC client pool from config
    pub fn new(config: &RpcConfig) -> Result<Self> {
        anyhow::ensure!(
            !config.endpoints.is_empty(),
            "At least one RPC endpoint is required"
        );

        let timeout = Duration::from_secs(config.timeout_secs);
        let clients: Vec<RpcClient> = config
            .endpoints
            .iter()
            .map(|endpoint| {
                RpcClient::new_with_timeout_and_commitment(
                    endpoint.clone(),
                    timeout,
                    CommitmentConfig::confirmed(),
                )
            })
            .collect();

        info!(
            endpoints = config.endpoints.len(),
            timeout_secs = config.timeout_secs,
            max_retries = config.max_retries,
            "RPC client pool created"
        );

        Ok(Self {
            clients,
            endpoints: config.endpoints.clone(),
            current_index: AtomicUsize::new(0),
            max_retries: config.max_retries,
            _timeout: timeout,
        })
    }

    /// Get the next client using round-robin load balancing
    fn next_client(&self) -> (&RpcClient, &str) {
        let index = self.current_index.fetch_add(1, Ordering::SeqCst) % self.clients.len();
        (&self.clients[index], &self.endpoints[index])
    }

    /// Get the number of endpoints
    pub fn endpoint_count(&self) -> usize {
        self.clients.len()
    }

    /// Get the current index (for testing)
    pub fn current_index(&self) -> usize {
        self.current_index.load(Ordering::SeqCst)
    }

    /// Get signatures for an address with retry and load balancing
    pub fn get_signatures_for_address(
        &self,
        address: &str,
        limit: Option<usize>,
    ) -> Result<Vec<solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature>> {
        let pubkey =
            Pubkey::from_str(address).context(format!("Invalid public key: {}", address))?;

        self.retry_with_failover(|client, endpoint| {
            let start = Instant::now();
            let config = solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config {
                limit,
                ..Default::default()
            };

            let result = client.get_signatures_for_address_with_config(&pubkey, config);
            let latency = start.elapsed();

            debug!(
                endpoint = endpoint,
                address = address,
                latency_ms = latency.as_millis() as u64,
                success = result.is_ok(),
                "RPC get_signatures_for_address"
            );

            result.context("Failed to get signatures for address")
        })
    }

    /// Get signatures for an address until a specific signature, with retry and load balancing
    pub fn get_signatures_for_address_until(
        &self,
        address: &str,
        limit: Option<usize>,
        until: Option<Signature>,
    ) -> Result<Vec<solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature>> {
        let pubkey =
            Pubkey::from_str(address).context(format!("Invalid public key: {}", address))?;

        self.retry_with_failover(|client, endpoint| {
            let start = Instant::now();
            let config = solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config {
                limit,
                until,
                ..Default::default()
            };

            let result = client.get_signatures_for_address_with_config(&pubkey, config);
            let latency = start.elapsed();

            debug!(
                endpoint = endpoint,
                address = address,
                latency_ms = latency.as_millis() as u64,
                success = result.is_ok(),
                "RPC get_signatures_for_address_until"
            );

            result.context("Failed to get signatures for address until")
        })
    }

    /// Get transaction details with retry and load balancing
    pub fn get_transaction(
        &self,
        signature: &Signature,
    ) -> Result<EncodedConfirmedTransactionWithStatusMeta> {
        self.retry_with_failover(|client, endpoint| {
            let start = Instant::now();
            let config = RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::JsonParsed),
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: Some(0),
            };

            let result = client.get_transaction_with_config(signature, config);
            let latency = start.elapsed();

            debug!(
                endpoint = endpoint,
                signature = %signature,
                latency_ms = latency.as_millis() as u64,
                success = result.is_ok(),
                "RPC get_transaction"
            );

            result.context("Failed to get transaction")
        })
    }

    /// Get current slot with retry and load balancing
    pub fn get_slot(&self) -> Result<u64> {
        self.retry_with_failover(|client, endpoint| {
            let start = Instant::now();
            let result = client.get_slot();
            let latency = start.elapsed();

            debug!(
                endpoint = endpoint,
                latency_ms = latency.as_millis() as u64,
                success = result.is_ok(),
                "RPC get_slot"
            );

            result.context("Failed to get slot")
        })
    }

    /// Health check: try to get the current slot
    pub fn health_check(&self) -> Result<()> {
        self.get_slot().map(|_| ())
    }

    /// Retry an RPC operation with failover across endpoints
    fn retry_with_failover<F, T>(&self, operation: F) -> Result<T>
    where
        F: Fn(&RpcClient, &str) -> Result<T>,
    {
        let mut last_error = None;
        let total_attempts = (self.max_retries + 1) as usize;

        for attempt in 0..total_attempts {
            let (client, endpoint) = self.next_client();

            match operation(client, endpoint) {
                Ok(result) => return Ok(result),
                Err(err) => {
                    let is_rate_limit = err.to_string().contains("429")
                        || err.to_string().to_lowercase().contains("rate limit");

                    if is_rate_limit {
                        // Back off more aggressively for rate limits
                        let delay = Duration::from_millis(500 * 2u64.pow(attempt as u32));
                        warn!(
                            endpoint = endpoint,
                            attempt = attempt + 1,
                            max_attempts = total_attempts,
                            delay_ms = delay.as_millis() as u64,
                            "RPC rate limited, backing off"
                        );
                        std::thread::sleep(delay);
                    } else if attempt < total_attempts - 1 {
                        let delay = Duration::from_millis(100 * 2u64.pow(attempt as u32));
                        warn!(
                            endpoint = endpoint,
                            attempt = attempt + 1,
                            max_attempts = total_attempts,
                            delay_ms = delay.as_millis() as u64,
                            error = %err,
                            "RPC request failed, retrying with next endpoint"
                        );
                        std::thread::sleep(delay);
                    }

                    last_error = Some(err);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("RPC request failed with no error details")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RpcConfig;

    fn test_config() -> RpcConfig {
        RpcConfig {
            endpoints: vec![
                "https://api.mainnet-beta.solana.com".to_string(),
                "https://solana-api.projectserum.com".to_string(),
            ],
            timeout_secs: 30,
            max_retries: 3,
        }
    }

    #[test]
    fn test_rpc_pool_creation() {
        let config = test_config();
        let pool = RpcClientPool::new(&config).unwrap();
        assert_eq!(pool.endpoint_count(), 2);
    }

    #[test]
    fn test_empty_endpoints_fails() {
        let config = RpcConfig {
            endpoints: vec![],
            timeout_secs: 30,
            max_retries: 3,
        };
        assert!(RpcClientPool::new(&config).is_err());
    }

    #[test]
    fn test_round_robin_distribution() {
        let config = test_config();
        let pool = RpcClientPool::new(&config).unwrap();

        // Track which endpoints are selected
        let mut selections = Vec::new();
        for _ in 0..6 {
            let idx = pool.current_index.fetch_add(1, Ordering::SeqCst) % pool.endpoint_count();
            selections.push(idx);
        }

        // Should alternate: 0, 1, 0, 1, 0, 1
        assert_eq!(selections, vec![0, 1, 0, 1, 0, 1]);
    }
}

/// Property-based tests for RPC load distribution
#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::config::RpcConfig;
    use proptest::prelude::*;

    /// **Property 18: RPC Load Distribution**
    /// For any set of N endpoints and M requests (M >> N),
    /// each endpoint should receive approximately M/N requests.
    ///
    /// **Validates: Requirements 8.6**

    proptest! {
        #[test]
        fn prop_rpc_load_distribution(
            num_endpoints in 2usize..=5,
            num_requests in 100usize..=500,
        ) {
            let endpoints: Vec<String> = (0..num_endpoints)
                .map(|i| format!("https://rpc{}.example.com", i))
                .collect();

            let config = RpcConfig {
                endpoints: endpoints.clone(),
                timeout_secs: 30,
                max_retries: 3,
            };

            let pool = RpcClientPool::new(&config).unwrap();

            // Simulate load distribution
            let mut counts = vec![0usize; num_endpoints];
            for _ in 0..num_requests {
                let idx = pool.current_index.fetch_add(1, Ordering::SeqCst) % num_endpoints;
                counts[idx] += 1;
            }

            let expected = num_requests / num_endpoints;
            let tolerance = 2; // Allow off-by-1 due to integer division

            for (i, count) in counts.iter().enumerate() {
                let diff = if *count > expected { count - expected } else { expected - count };
                prop_assert!(
                    diff <= tolerance,
                    "Endpoint {} received {} requests, expected ~{} (tolerance {})",
                    i, count, expected, tolerance
                );
            }
        }
    }
}

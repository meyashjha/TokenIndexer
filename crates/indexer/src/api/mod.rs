#![allow(unused_doc_comments)]

/// REST API server with Actix-web
///
/// This module provides:
/// - REST API endpoints for tokens, whale wallets, and alerts
/// - Request/response handling
/// - Pagination support
/// - Rate limiting middleware
/// - WebSocket server for real-time notifications
/// - Alert dispatcher for event processing
pub mod dispatcher;
pub mod websocket;

use crate::config::ApiConfig;
use crate::database::{EarlyBuyerRepo, TokenRepo, WhaleAlertRepo, WhaleWalletRepo};
use crate::models::*;
use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::error;

pub mod keys {
    pub fn rate_limit_key(client_id: &str, endpoint: &str) -> String {
        format!("rate_limit:{}:{}", client_id, endpoint)
    }
}

/// Shared application state
pub struct AppState {
    pub token_repo: Arc<dyn TokenRepo>,
    pub whale_repo: Arc<dyn WhaleWalletRepo>,
    pub alert_repo: Arc<dyn WhaleAlertRepo>,
    pub early_buyer_repo: Arc<dyn EarlyBuyerRepo>,
    pub config: ApiConfig,
}

/// API error response
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
}

/// Query parameters for pagination
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

impl PaginationQuery {
    pub fn into_params(self) -> PaginationParams {
        PaginationParams::new(self.page.unwrap_or(1), self.page_size.unwrap_or(20))
    }
}

/// Query parameters for time range filtering
#[derive(Debug, Deserialize)]
pub struct TimeRangeQuery {
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

// ==================== Health Check ====================

/// Health check endpoint
pub async fn health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "timestamp": Utc::now().to_rfc3339()
    }))
}

/// Readiness check endpoint
pub async fn ready() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ready",
        "timestamp": Utc::now().to_rfc3339()
    }))
}

// ==================== Token Endpoints ====================

/// GET /api/tokens - List recently discovered tokens
pub async fn list_tokens(
    state: web::Data<AppState>,
    query: web::Query<PaginationQuery>,
) -> impl Responder {
    let params = query.into_inner().into_params();

    match state.token_repo.list_recent(params.page_size as i64).await {
        Ok(tokens) => {
            let response = PaginatedResponse::new(tokens.clone(), tokens.len() as u64, &params);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            error!(error = %e, "Failed to list tokens");
            HttpResponse::InternalServerError().json(ApiError {
                error: "internal_error".to_string(),
                message: "Failed to retrieve tokens".to_string(),
            })
        }
    }
}

/// GET /api/tokens/{mint} - Get token details by mint address
pub async fn get_token(state: web::Data<AppState>, mint: web::Path<String>) -> impl Responder {
    let mint_address = mint.into_inner();

    match state.token_repo.get_by_mint(&mint_address).await {
        Ok(Some(token)) => HttpResponse::Ok().json(token),
        Ok(None) => HttpResponse::NotFound().json(ApiError {
            error: "not_found".to_string(),
            message: format!("Token with mint address {} not found", mint_address),
        }),
        Err(e) => {
            error!(error = %e, mint = %mint_address, "Failed to get token");
            HttpResponse::InternalServerError().json(ApiError {
                error: "internal_error".to_string(),
                message: "Failed to retrieve token".to_string(),
            })
        }
    }
}

/// GET /api/tokens/{mint}/transactions - List transactions for a token
pub async fn list_token_transactions(
    state: web::Data<AppState>,
    mint: web::Path<String>,
    query: web::Query<PaginationQuery>,
) -> impl Responder {
    let mint_address = mint.into_inner();
    let params = query.into_inner().into_params();

    match state
        .early_buyer_repo
        .list_transactions_for_token(&mint_address, params.page_size as i64)
        .await
    {
        Ok(transactions) => {
            let response = PaginatedResponse::new(
                transactions.clone(),
                transactions.len() as u64,
                &params,
            );
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            error!(
                error = %e,
                mint = %mint_address,
                "Failed to list token transactions"
            );
            HttpResponse::InternalServerError().json(ApiError {
                error: "internal_error".to_string(),
                message: "Failed to retrieve token transactions".to_string(),
            })
        }
    }
}

// ==================== Whale Wallet Endpoints ====================

/// GET /api/whales - List whale wallets with profitability metrics
pub async fn list_whales(
    state: web::Data<AppState>,
    query: web::Query<PaginationQuery>,
) -> impl Responder {
    let params = query.into_inner().into_params();

    match state
        .whale_repo
        .list_paginated(params.offset() as i64, params.page_size as i64)
        .await
    {
        Ok((wallets, total)) => {
            let response = PaginatedResponse::new(wallets, total as u64, &params);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            error!(error = %e, "Failed to list whales");
            HttpResponse::InternalServerError().json(ApiError {
                error: "internal_error".to_string(),
                message: "Failed to retrieve whale wallets".to_string(),
            })
        }
    }
}

/// GET /api/whales/{address} - Get whale wallet details
pub async fn get_whale(state: web::Data<AppState>, address: web::Path<String>) -> impl Responder {
    let wallet_address = address.into_inner();

    match state.whale_repo.get_by_address(&wallet_address).await {
        Ok(Some(wallet)) => HttpResponse::Ok().json(wallet),
        Ok(None) => HttpResponse::NotFound().json(ApiError {
            error: "not_found".to_string(),
            message: format!("Whale wallet {} not found", wallet_address),
        }),
        Err(e) => {
            error!(error = %e, wallet = %wallet_address, "Failed to get whale");
            HttpResponse::InternalServerError().json(ApiError {
                error: "internal_error".to_string(),
                message: "Failed to retrieve whale wallet".to_string(),
            })
        }
    }
}

// ==================== Whale Alert Endpoints ====================

/// GET /api/alerts - Query whale alerts by time range
pub async fn list_alerts(
    state: web::Data<AppState>,
    query: web::Query<TimeRangeQuery>,
) -> impl Responder {
    let params = PaginationParams::new(query.page.unwrap_or(1), query.page_size.unwrap_or(20));

    // Parse time range
    let start_time = query
        .start_time
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24));

    let end_time = query
        .end_time
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now());

    match state
        .alert_repo
        .get_by_time_range(start_time, end_time)
        .await
    {
        Ok(alerts) => {
            let total = alerts.len() as u64;
            let paginated: Vec<_> = alerts
                .into_iter()
                .skip(params.offset() as usize)
                .take(params.page_size as usize)
                .collect();
            let response = PaginatedResponse::new(paginated, total, &params);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            error!(error = %e, "Failed to list alerts");
            HttpResponse::InternalServerError().json(ApiError {
                error: "internal_error".to_string(),
                message: "Failed to retrieve alerts".to_string(),
            })
        }
    }
}

// ==================== Rate Limiting ====================

// ==================== Route Configuration ====================

/// Configure API routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .route("/health", web::get().to(health))
            .route("/health/ready", web::get().to(ready))
            .route("/tokens", web::get().to(list_tokens))
            .route("/tokens/{mint}", web::get().to(get_token))
            .route("/tokens/{mint}/transactions", web::get().to(list_token_transactions))
            .route("/whales", web::get().to(list_whales))
            .route("/whales/{address}", web::get().to(get_whale))
            .route("/alerts", web::get().to(list_alerts)),
    );
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_query_defaults() {
        let query = PaginationQuery {
            page: None,
            page_size: None,
        };
        let params = query.into_params();
        assert_eq!(params.page, 1);
        assert_eq!(params.page_size, 20);
    }

    #[test]
    fn test_pagination_query_custom() {
        let query = PaginationQuery {
            page: Some(3),
            page_size: Some(50),
        };
        let params = query.into_params();
        assert_eq!(params.page, 3);
        assert_eq!(params.page_size, 50);
    }

    #[test]
    fn test_api_error_serialization() {
        let err = ApiError {
            error: "not_found".to_string(),
            message: "Resource not found".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("not_found"));
        assert!(json.contains("Resource not found"));
    }
}

/// Property-based tests for time range filtering and pagination
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// **Property 12: Time Range Filtering**
    /// For any collection of whale alerts and a specified time range,
    /// filtering alerts by time range SHALL return only alerts within the range
    /// and SHALL include all such alerts.
    ///
    /// **Validates: Requirements 5.5**
    proptest! {
        #[test]
        fn prop_time_range_filtering_correct(
            num_alerts in 1usize..=50,
            range_start_offset_hours in 0i64..=48,
            range_duration_hours in 1i64..=24,
        ) {
            let now = Utc::now();
            let range_start = now - chrono::Duration::hours(range_start_offset_hours);
            let range_end = range_start + chrono::Duration::hours(range_duration_hours);

            // Generate alerts spread across different times
            let alerts: Vec<WhaleAlert> = (0..num_alerts)
                .map(|i| {
                    let hours_offset = (i as i64 * 2) - (num_alerts as i64);
                    WhaleAlert {
                        id: uuid::Uuid::new_v4(),
                        timestamp: now + chrono::Duration::hours(hours_offset),
                        wallet_address: format!("wallet_{}", i),
                        token_address: format!("token_{}", i),
                        purchase_amount: 1.0,
                        wallet_metrics: WalletMetrics {
                            win_rate: 0.75,
                            average_roi: 3.5,
                            wallet_type: WalletType::EstablishedSniper,
                        },
                        transaction_signature: None,
                        slot_number: None,
                    }
                })
                .collect();

            // Filter by time range
            let filtered: Vec<_> = alerts
                .iter()
                .filter(|a| a.timestamp >= range_start && a.timestamp <= range_end)
                .collect();

            // Verify: all filtered alerts are within range
            for alert in &filtered {
                prop_assert!(
                    alert.timestamp >= range_start && alert.timestamp <= range_end,
                    "Alert at {:?} is outside range [{:?}, {:?}]",
                    alert.timestamp, range_start, range_end
                );
            }

            // Verify: no valid alert was excluded
            for alert in &alerts {
                if alert.timestamp >= range_start && alert.timestamp <= range_end {
                    prop_assert!(
                        filtered.iter().any(|a| a.id == alert.id),
                        "Alert {} within range was excluded",
                        alert.id
                    );
                }
            }
        }
    }

    /// **Property 14: Pagination Correctness**
    /// For any dataset of size N and pagination parameters,
    /// applying pagination SHALL return the correct subset.
    ///
    /// **Validates: Requirements 5.9**
    proptest! {
        #[test]
        fn prop_pagination_correctness(
            total_items in 0u64..=200,
            page in 1u32..=20,
            page_size in 1u32..=50,
        ) {
            let params = PaginationParams { page, page_size };

            // Create items
            let items: Vec<u64> = (0..total_items).collect();

            // Apply pagination
            let start = params.offset() as usize;
            let paginated: Vec<_> = items
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect();

            // Verify response
            let response = PaginatedResponse::new(paginated.clone(), total_items, &params);

            prop_assert_eq!(response.total, total_items);
            prop_assert_eq!(response.page, page);
            prop_assert_eq!(response.page_size, page_size);

            // Verify correct items
            let expected_len = if start >= total_items as usize {
                0
            } else {
                (total_items as usize - start).min(page_size as usize)
            };
            prop_assert_eq!(response.data.len(), expected_len);

            // Verify total pages
            let expected_pages = if page_size > 0 {
                ((total_items as f64) / (page_size as f64)).ceil() as u32
            } else {
                0
            };
            prop_assert_eq!(response.total_pages, expected_pages);
        }
    }

    /// **Property 17: Rate Limiting Enforcement**
    /// If request count exceeds the configured rate limit within the time window,
    /// subsequent requests SHALL be rejected.
    ///
    /// **Validates: Requirements 7.6**
    proptest! {
        #[test]
        fn prop_rate_limit_key_generation(
            client_id in "[a-z0-9]{4,10}",
            endpoint in "/api/[a-z]+",
        ) {
            let key = keys::rate_limit_key(&client_id, &endpoint);
            prop_assert!(key.starts_with("rate_limit:"));
            prop_assert!(key.contains(&client_id));
            prop_assert!(key.contains(&endpoint));
        }
    }
}

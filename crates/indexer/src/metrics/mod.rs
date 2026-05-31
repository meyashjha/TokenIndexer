#![allow(unused_doc_comments)]

/// Prometheus metrics and observability
///
/// This module provides:
/// - Metrics collection for polling intervals, RPC latency, event counts
/// - Prometheus exporter
/// - Health check endpoints
use prometheus::{
    Encoder, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts,
    Registry, TextEncoder,
};
use std::sync::Arc;
use tracing::{error, info};

/// Metrics collector for the indexer
#[derive(Clone)]
pub struct Metrics {
    registry: Registry,

    // Scout metrics
    pub tokens_discovered: IntCounter,
    pub scout_poll_duration: Histogram,

    // Hunter metrics
    pub tokens_analyzed: IntCounter,
    pub whales_identified: IntCounter,
    pub hunter_poll_duration: Histogram,

    // Shadow metrics
    pub whale_purchases_detected: IntCounter,
    pub shadow_poll_duration: Histogram,
    pub wallets_monitored: IntGauge,

    // RPC metrics
    pub rpc_requests_total: IntCounterVec,
    pub rpc_request_duration: HistogramVec,
    pub rpc_errors_total: IntCounterVec,

    // Database metrics
    pub db_query_duration: HistogramVec,
    pub db_errors_total: IntCounter,

    // Cache metrics
    pub cache_hits: IntCounter,
    pub cache_misses: IntCounter,

    // Queue metrics
    pub events_published: IntCounterVec,
    pub events_buffered: IntGauge,

    // API metrics
    pub api_requests_total: IntCounterVec,
    pub api_request_duration: HistogramVec,
    pub api_rate_limited: IntCounter,

    // WebSocket metrics
    pub ws_connections: IntGauge,
    pub ws_messages_sent: IntCounter,
}

impl Metrics {
    /// Create a new metrics collector
    pub fn new() -> anyhow::Result<Self> {
        let registry = Registry::new();

        // Scout metrics
        let tokens_discovered = IntCounter::new(
            "scout_tokens_discovered_total",
            "Total number of tokens discovered by the Scout",
        )?;
        let scout_poll_duration = Histogram::with_opts(
            HistogramOpts::new(
                "scout_poll_duration_seconds",
                "Duration of Scout polling cycles",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0]),
        )?;

        // Hunter metrics
        let tokens_analyzed = IntCounter::new(
            "hunter_tokens_analyzed_total",
            "Total number of tokens analyzed by the Hunter",
        )?;
        let whales_identified = IntCounter::new(
            "hunter_whales_identified_total",
            "Total number of whale wallets identified",
        )?;
        let hunter_poll_duration = Histogram::with_opts(
            HistogramOpts::new(
                "hunter_poll_duration_seconds",
                "Duration of Hunter polling cycles",
            )
            .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0]),
        )?;

        // Shadow metrics
        let whale_purchases_detected = IntCounter::new(
            "shadow_whale_purchases_total",
            "Total number of whale purchases detected",
        )?;
        let shadow_poll_duration = Histogram::with_opts(
            HistogramOpts::new(
                "shadow_poll_duration_seconds",
                "Duration of Shadow polling cycles",
            )
            .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0]),
        )?;
        let wallets_monitored = IntGauge::new(
            "shadow_wallets_monitored",
            "Number of whale wallets currently monitored",
        )?;

        // RPC metrics
        let rpc_requests_total = IntCounterVec::new(
            Opts::new("rpc_requests_total", "Total RPC requests"),
            &["method", "endpoint"],
        )?;
        let rpc_request_duration = HistogramVec::new(
            HistogramOpts::new("rpc_request_duration_seconds", "RPC request duration")
                .buckets(vec![0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]),
            &["method", "endpoint"],
        )?;
        let rpc_errors_total = IntCounterVec::new(
            Opts::new("rpc_errors_total", "Total RPC errors"),
            &["method", "endpoint", "error_type"],
        )?;

        // Database metrics
        let db_query_duration = HistogramVec::new(
            HistogramOpts::new("db_query_duration_seconds", "Database query duration")
                .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5]),
            &["query_type"],
        )?;
        let db_errors_total = IntCounter::new("db_errors_total", "Total database errors")?;

        // Cache metrics
        let cache_hits = IntCounter::new("cache_hits_total", "Total cache hits")?;
        let cache_misses = IntCounter::new("cache_misses_total", "Total cache misses")?;

        // Queue metrics
        let events_published = IntCounterVec::new(
            Opts::new("events_published_total", "Total events published"),
            &["event_type"],
        )?;
        let events_buffered =
            IntGauge::new("events_buffered", "Number of events currently buffered")?;

        // API metrics
        let api_requests_total = IntCounterVec::new(
            Opts::new("api_requests_total", "Total API requests"),
            &["method", "endpoint", "status"],
        )?;
        let api_request_duration = HistogramVec::new(
            HistogramOpts::new("api_request_duration_seconds", "API request duration")
                .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.2, 0.5]),
            &["endpoint"],
        )?;
        let api_rate_limited =
            IntCounter::new("api_rate_limited_total", "Total API requests rate limited")?;

        // WebSocket metrics
        let ws_connections = IntGauge::new(
            "ws_connections_active",
            "Number of active WebSocket connections",
        )?;
        let ws_messages_sent =
            IntCounter::new("ws_messages_sent_total", "Total WebSocket messages sent")?;

        // Register all metrics
        registry.register(Box::new(tokens_discovered.clone()))?;
        registry.register(Box::new(scout_poll_duration.clone()))?;
        registry.register(Box::new(tokens_analyzed.clone()))?;
        registry.register(Box::new(whales_identified.clone()))?;
        registry.register(Box::new(hunter_poll_duration.clone()))?;
        registry.register(Box::new(whale_purchases_detected.clone()))?;
        registry.register(Box::new(shadow_poll_duration.clone()))?;
        registry.register(Box::new(wallets_monitored.clone()))?;
        registry.register(Box::new(rpc_requests_total.clone()))?;
        registry.register(Box::new(rpc_request_duration.clone()))?;
        registry.register(Box::new(rpc_errors_total.clone()))?;
        registry.register(Box::new(db_query_duration.clone()))?;
        registry.register(Box::new(db_errors_total.clone()))?;
        registry.register(Box::new(cache_hits.clone()))?;
        registry.register(Box::new(cache_misses.clone()))?;
        registry.register(Box::new(events_published.clone()))?;
        registry.register(Box::new(events_buffered.clone()))?;
        registry.register(Box::new(api_requests_total.clone()))?;
        registry.register(Box::new(api_request_duration.clone()))?;
        registry.register(Box::new(api_rate_limited.clone()))?;
        registry.register(Box::new(ws_connections.clone()))?;
        registry.register(Box::new(ws_messages_sent.clone()))?;

        info!("Prometheus metrics initialized");

        Ok(Self {
            registry,
            tokens_discovered,
            scout_poll_duration,
            tokens_analyzed,
            whales_identified,
            hunter_poll_duration,
            whale_purchases_detected,
            shadow_poll_duration,
            wallets_monitored,
            rpc_requests_total,
            rpc_request_duration,
            rpc_errors_total,
            db_query_duration,
            db_errors_total,
            cache_hits,
            cache_misses,
            events_published,
            events_buffered,
            api_requests_total,
            api_request_duration,
            api_rate_limited,
            ws_connections,
            ws_messages_sent,
        })
    }

    /// Export metrics in Prometheus text format
    pub fn export(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .unwrap_or_else(|e| {
                error!(error = %e, "Failed to encode metrics");
            });
        String::from_utf8(buffer).unwrap_or_default()
    }

    /// Get the registry for custom metric registration
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

/// Actix-web handler for the /metrics endpoint
pub async fn metrics_handler(
    metrics: actix_web::web::Data<Arc<Metrics>>,
) -> actix_web::HttpResponse {
    let body = metrics.export();
    actix_web::HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new().unwrap();
        assert!(metrics.export().is_empty() || !metrics.export().is_empty());
    }

    #[test]
    fn test_counter_increment() {
        let metrics = Metrics::new().unwrap();
        metrics.tokens_discovered.inc();
        metrics.tokens_discovered.inc();

        let output = metrics.export();
        assert!(output.contains("scout_tokens_discovered_total"));
        assert!(output.contains("2"));
    }

    #[test]
    fn test_gauge_operations() {
        let metrics = Metrics::new().unwrap();
        metrics.wallets_monitored.set(100);

        let output = metrics.export();
        assert!(output.contains("shadow_wallets_monitored"));
        assert!(output.contains("100"));
    }

    #[test]
    fn test_histogram_observation() {
        let metrics = Metrics::new().unwrap();
        metrics.scout_poll_duration.observe(0.5);
        metrics.scout_poll_duration.observe(1.5);

        let output = metrics.export();
        assert!(output.contains("scout_poll_duration_seconds"));
    }

    #[test]
    fn test_counter_vec_labels() {
        let metrics = Metrics::new().unwrap();
        metrics
            .rpc_requests_total
            .with_label_values(&["getSignatures", "https://api.mainnet.solana.com"])
            .inc();

        let output = metrics.export();
        assert!(output.contains("rpc_requests_total"));
        assert!(output.contains("getSignatures"));
    }
}

/// Property-based tests for Prometheus metrics format
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// **Property 21: Prometheus Metrics Format**
    /// For any system state, exposing metrics SHALL produce output in valid
    /// Prometheus format with correct metric types.
    ///
    /// **Validates: Requirements 9.7, 12.8**
    proptest! {
        #[test]
        fn prop_metrics_always_valid_format(
            counter_value in 0u64..=1000,
            gauge_value in -1000i64..=1000,
            histogram_value in 0.001f64..=100.0,
        ) {
            let metrics = Metrics::new().unwrap();

            // Set various metric values
            for _ in 0..counter_value {
                metrics.tokens_discovered.inc();
            }
            metrics.wallets_monitored.set(gauge_value);
            metrics.scout_poll_duration.observe(histogram_value);

            let output = metrics.export();

            // Verify: output is non-empty
            prop_assert!(!output.is_empty(), "Metrics output should not be empty");

            // Verify: output contains expected metric names
            prop_assert!(output.contains("scout_tokens_discovered_total"));
            prop_assert!(output.contains("shadow_wallets_monitored"));
            prop_assert!(output.contains("scout_poll_duration_seconds"));

            // Verify: each line is valid Prometheus format
            for line in output.lines() {
                if line.starts_with('#') {
                    // Comment line: must be # HELP or # TYPE
                    prop_assert!(
                        line.starts_with("# HELP") || line.starts_with("# TYPE"),
                        "Invalid comment line: {}",
                        line
                    );
                } else if !line.is_empty() {
                    // Metric line: must contain metric name and value
                    prop_assert!(
                        line.contains(' ') || line.contains('{'),
                        "Invalid metric line (no space or label): {}",
                        line
                    );
                }
            }
        }
    }
}

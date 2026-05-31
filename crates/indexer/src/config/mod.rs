/// Configuration management for the Solana token indexer
///
/// This module handles loading and validating configuration from
/// environment variables and configuration files.
use anyhow::{Context, Result};
use config::{Config as ConfigBuilder, Environment, File};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub scout: ScoutConfig,
    pub hunter: HunterConfig,
    pub shadow: ShadowConfig,
    pub rpc: RpcConfig,
    pub database: DatabaseConfig,

    pub api: ApiConfig,
    pub websocket: WebSocketConfig,
    pub logging: LoggingConfig,
}

/// Configuration for The Scout (launchpad indexer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoutConfig {
    pub polling_interval_secs: u64,
    pub pumpfun_program_id: String,
    pub raydium_program_id: String,
}

/// Configuration for The Hunter (token analyst)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HunterConfig {
    pub polling_interval_secs: u64,
    pub token_maturity_minutes: u64,
    pub early_buyer_window_secs: u64,
    pub max_signatures_per_token: usize,
    pub whale_win_rate_threshold: f64,
    pub whale_roi_threshold: f64,
}

/// Configuration for The Shadow (whale tracker)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowConfig {
    pub polling_interval_secs: u64,
    pub max_concurrent_wallets: usize,
}

/// Configuration for RPC client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub endpoints: Vec<String>,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

/// Configuration for database connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_secs: u64,
}

/// Configuration for REST API server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub rate_limit_requests: u32,
    pub rate_limit_window_secs: u64,
}

/// Configuration for WebSocket server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub heartbeat_interval_secs: u64,
}

/// Configuration for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: LogFormat,
}

/// Log output format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    Json,
    Pretty,
}

impl Config {
    /// Load configuration from environment variables and config files
    ///
    /// Configuration is loaded in the following order (later sources override earlier ones):
    /// 1. Default config file (config.toml)
    /// 2. Environment-specific config file (config.{ENV}.toml)
    /// 3. Environment variables (prefixed with APP_)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use solana_indexer::config::Config;
    ///
    /// let config = Config::load().expect("Failed to load configuration");
    /// ```
    pub fn load() -> Result<Self> {
        let env = std::env::var("RUN_ENV").unwrap_or_else(|_| "development".to_string());

        let config = ConfigBuilder::builder()
            // Start with default config file
            .add_source(File::with_name("config").required(false))
            // Add environment-specific config file
            .add_source(File::with_name(&format!("config.{}", env)).required(false))
            // Add environment variables (with APP_ prefix)
            // Example: APP_SCOUT__POLLING_INTERVAL_SECS=10
            .add_source(
                Environment::with_prefix("APP")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()
            .context("Failed to build configuration")?;

        let config: Config = config
            .try_deserialize()
            .context("Failed to deserialize configuration")?;

        // Validate the loaded configuration
        config.validate()?;

        Ok(config)
    }

    /// Validate configuration values
    ///
    /// Ensures all required fields are present and values are within acceptable ranges.
    /// This method is automatically called by `load()`.
    ///
    /// # Errors
    ///
    /// Returns an error if any configuration value is invalid or out of range.
    pub fn validate(&self) -> Result<()> {
        // Validate Scout configuration
        self.validate_scout()?;

        // Validate Hunter configuration
        self.validate_hunter()?;

        // Validate Shadow configuration
        self.validate_shadow()?;

        // Validate RPC configuration
        self.validate_rpc()?;

        // Validate Database configuration
        self.validate_database()?;

        // Validate API configuration
        self.validate_api()?;

        // Validate WebSocket configuration
        self.validate_websocket()?;

        // Validate Logging configuration
        self.validate_logging()?;

        Ok(())
    }

    fn validate_scout(&self) -> Result<()> {
        anyhow::ensure!(
            self.scout.polling_interval_secs >= 1 && self.scout.polling_interval_secs <= 3600,
            "Scout polling_interval_secs must be between 1 and 3600, got {}",
            self.scout.polling_interval_secs
        );

        anyhow::ensure!(
            !self.scout.pumpfun_program_id.is_empty(),
            "Scout pumpfun_program_id cannot be empty"
        );

        anyhow::ensure!(
            !self.scout.raydium_program_id.is_empty(),
            "Scout raydium_program_id cannot be empty"
        );

        Ok(())
    }

    fn validate_hunter(&self) -> Result<()> {
        anyhow::ensure!(
            self.hunter.polling_interval_secs >= 10 && self.hunter.polling_interval_secs <= 300,
            "Hunter polling_interval_secs must be between 10 and 300, got {}",
            self.hunter.polling_interval_secs
        );

        anyhow::ensure!(
            self.hunter.token_maturity_minutes >= 1,
            "Hunter token_maturity_minutes must be at least 1, got {}",
            self.hunter.token_maturity_minutes
        );

        anyhow::ensure!(
            self.hunter.early_buyer_window_secs >= 10 && self.hunter.early_buyer_window_secs <= 300,
            "Hunter early_buyer_window_secs must be between 10 and 300, got {}",
            self.hunter.early_buyer_window_secs
        );

        anyhow::ensure!(
            self.hunter.max_signatures_per_token >= 10
                && self.hunter.max_signatures_per_token <= 1000,
            "Hunter max_signatures_per_token must be between 10 and 1000, got {}",
            self.hunter.max_signatures_per_token
        );

        anyhow::ensure!(
            self.hunter.whale_win_rate_threshold >= 0.0
                && self.hunter.whale_win_rate_threshold <= 1.0,
            "Hunter whale_win_rate_threshold must be between 0.0 and 1.0, got {}",
            self.hunter.whale_win_rate_threshold
        );

        anyhow::ensure!(
            self.hunter.whale_roi_threshold >= 0.0,
            "Hunter whale_roi_threshold must be non-negative, got {}",
            self.hunter.whale_roi_threshold
        );

        Ok(())
    }

    fn validate_shadow(&self) -> Result<()> {
        anyhow::ensure!(
            self.shadow.polling_interval_secs >= 1 && self.shadow.polling_interval_secs <= 60,
            "Shadow polling_interval_secs must be between 1 and 60, got {}",
            self.shadow.polling_interval_secs
        );

        anyhow::ensure!(
            self.shadow.max_concurrent_wallets >= 1 && self.shadow.max_concurrent_wallets <= 10000,
            "Shadow max_concurrent_wallets must be between 1 and 10000, got {}",
            self.shadow.max_concurrent_wallets
        );

        Ok(())
    }

    fn validate_rpc(&self) -> Result<()> {
        anyhow::ensure!(
            !self.rpc.endpoints.is_empty(),
            "RPC endpoints list cannot be empty"
        );

        for endpoint in &self.rpc.endpoints {
            anyhow::ensure!(
                endpoint.starts_with("http://") || endpoint.starts_with("https://"),
                "RPC endpoint must start with http:// or https://, got: {}",
                endpoint
            );
        }

        anyhow::ensure!(
            self.rpc.timeout_secs >= 5 && self.rpc.timeout_secs <= 300,
            "RPC timeout_secs must be between 5 and 300, got {}",
            self.rpc.timeout_secs
        );

        anyhow::ensure!(
            self.rpc.max_retries <= 10,
            "RPC max_retries must be between 0 and 10, got {}",
            self.rpc.max_retries
        );

        Ok(())
    }

    fn validate_database(&self) -> Result<()> {
        anyhow::ensure!(
            !self.database.url.is_empty(),
            "Database URL cannot be empty"
        );

        anyhow::ensure!(
            self.database.url.starts_with("postgresql://")
                || self.database.url.starts_with("postgres://"),
            "Database URL must start with postgresql:// or postgres://, got: {}",
            self.database.url
        );

        anyhow::ensure!(
            self.database.max_connections >= 1 && self.database.max_connections <= 1000,
            "Database max_connections must be between 1 and 1000, got {}",
            self.database.max_connections
        );

        anyhow::ensure!(
            self.database.min_connections >= 1
                && self.database.min_connections <= self.database.max_connections,
            "Database min_connections must be between 1 and max_connections ({}), got {}",
            self.database.max_connections,
            self.database.min_connections
        );

        anyhow::ensure!(
            self.database.connection_timeout_secs >= 1
                && self.database.connection_timeout_secs <= 300,
            "Database connection_timeout_secs must be between 1 and 300, got {}",
            self.database.connection_timeout_secs
        );

        Ok(())
    }

    fn validate_api(&self) -> Result<()> {
        anyhow::ensure!(!self.api.host.is_empty(), "API host cannot be empty");

        anyhow::ensure!(
            self.api.port > 0,
            "API port must be greater than 0, got {}",
            self.api.port
        );

        anyhow::ensure!(
            self.api.rate_limit_requests >= 1,
            "API rate_limit_requests must be at least 1, got {}",
            self.api.rate_limit_requests
        );

        anyhow::ensure!(
            self.api.rate_limit_window_secs >= 1,
            "API rate_limit_window_secs must be at least 1, got {}",
            self.api.rate_limit_window_secs
        );

        Ok(())
    }

    fn validate_websocket(&self) -> Result<()> {
        anyhow::ensure!(
            !self.websocket.host.is_empty(),
            "WebSocket host cannot be empty"
        );

        anyhow::ensure!(
            self.websocket.port > 0,
            "WebSocket port must be greater than 0, got {}",
            self.websocket.port
        );

        anyhow::ensure!(
            self.websocket.max_connections >= 1,
            "WebSocket max_connections must be at least 1, got {}",
            self.websocket.max_connections
        );

        anyhow::ensure!(
            self.websocket.heartbeat_interval_secs >= 1
                && self.websocket.heartbeat_interval_secs <= 300,
            "WebSocket heartbeat_interval_secs must be between 1 and 300, got {}",
            self.websocket.heartbeat_interval_secs
        );

        Ok(())
    }

    fn validate_logging(&self) -> Result<()> {
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        let level_lower = self.logging.level.to_lowercase();

        anyhow::ensure!(
            valid_levels.contains(&level_lower.as_str()),
            "Logging level must be one of {:?}, got: {}",
            valid_levels,
            self.logging.level
        );

        Ok(())
    }
}

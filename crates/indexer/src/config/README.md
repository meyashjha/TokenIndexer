# Configuration Module

This module handles loading and validating configuration for the Solana Three-Tier Token Indexer.

## Overview

The configuration system supports multiple sources with the following precedence (later sources override earlier ones):

1. Default config file (`config.toml`)
2. Environment-specific config file (`config.{ENV}.toml` where ENV is from `RUN_ENV` environment variable)
3. Environment variables (prefixed with `APP_`)

## Configuration Structure

The configuration is organized into the following sections:

### Scout Configuration
- `polling_interval_secs`: Polling interval for launchpad monitoring (1-60 seconds)
- `pumpfun_program_id`: Pump.fun program ID
- `raydium_program_id`: Raydium program ID

### Hunter Configuration
- `polling_interval_secs`: Polling interval for token analysis (10-300 seconds)
- `token_maturity_minutes`: Minimum token age before analysis (≥1 minute)
- `early_buyer_window_secs`: Time window for early buyer detection (10-300 seconds)
- `max_signatures_per_token`: Maximum signatures to fetch per token (10-1000)
- `whale_win_rate_threshold`: Win rate threshold for whale classification (0.0-1.0)
- `whale_roi_threshold`: ROI threshold for whale classification (≥0.0)

### Shadow Configuration
- `polling_interval_secs`: Polling interval for whale tracking (1-60 seconds)
- `max_concurrent_wallets`: Maximum concurrent wallets to monitor (1-10000)

### RPC Configuration
- `endpoints`: List of Solana RPC endpoint URLs (must start with http:// or https://)
- `timeout_secs`: RPC request timeout (5-300 seconds)
- `max_retries`: Maximum retry attempts (0-10)

### Database Configuration
- `url`: PostgreSQL connection URL (must start with postgresql:// or postgres://)
- `max_connections`: Maximum database connections (1-1000)
- `min_connections`: Minimum database connections (1-max_connections)
- `connection_timeout_secs`: Connection timeout (1-300 seconds)

### Redis Configuration
- `url`: Redis connection URL (must start with redis:// or rediss://)
- `pool_size`: Redis connection pool size (1-1000)
- `token_ttl_secs`: TTL for token cache entries (≥1 second)
- `whale_wallet_ttl_secs`: TTL for whale wallet cache entries (≥1 second)
- `token_details_ttl_secs`: TTL for token details cache entries (≥1 second)

### API Configuration
- `host`: API server host address
- `port`: API server port (>0)
- `rate_limit_requests`: Maximum requests per window (≥1)
- `rate_limit_window_secs`: Rate limit window duration (≥1 second)

### WebSocket Configuration
- `host`: WebSocket server host address
- `port`: WebSocket server port (>0)
- `max_connections`: Maximum concurrent WebSocket connections (≥1)
- `heartbeat_interval_secs`: WebSocket heartbeat interval (1-300 seconds)

### Logging Configuration
- `level`: Log level (trace, debug, info, warn, error - case insensitive)
- `format`: Log output format (Json or Pretty)

## Usage

### Loading Configuration from File

Create a `config.toml` file in the workspace root:

```toml
[scout]
polling_interval_secs = 5
pumpfun_program_id = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
raydium_program_id = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"

[hunter]
polling_interval_secs = 30
token_maturity_minutes = 30
early_buyer_window_secs = 60
max_signatures_per_token = 50
whale_win_rate_threshold = 0.70
whale_roi_threshold = 3.0

# ... other sections
```

Then load it in your code:

```rust
use solana_indexer::config::Config;

let config = Config::load()?;
```

### Using Environment Variables

Environment variables use the `APP_` prefix and double underscores (`__`) as separators:

```bash
export APP_SCOUT__POLLING_INTERVAL_SECS=10
export APP_HUNTER__WHALE_WIN_RATE_THRESHOLD=0.75
export APP_DATABASE__URL="postgresql://user:pass@localhost:5432/db"
export APP_LOGGING__LEVEL=debug
```

### Environment-Specific Configuration

Set the `RUN_ENV` environment variable to load environment-specific config:

```bash
export RUN_ENV=production
```

This will load `config.production.toml` in addition to `config.toml`.

## Validation

All configuration values are validated when loaded. Invalid configurations will cause the application to fail at startup with a clear error message indicating which parameter is invalid and why.

### Validation Rules

- All numeric ranges are enforced
- URL protocols are validated
- Required fields must be non-empty
- Relationships between fields are checked (e.g., min_connections ≤ max_connections)

## Error Handling

Configuration errors are returned as `anyhow::Result` with descriptive error messages:

```rust
match Config::load() {
    Ok(config) => {
        // Use config
    }
    Err(e) => {
        eprintln!("Configuration error: {}", e);
        std::process::exit(1);
    }
}
```

## Testing

The module includes comprehensive unit tests for validation logic. Run tests with:

```bash
cargo test --package solana-indexer --lib config::tests
```

## Requirements Mapping

This implementation satisfies the following requirements:

- **Requirement 11.1**: Load configuration from environment variables or configuration files
- **Requirement 11.2**: Support configurable polling intervals for Scout, Hunter, and Shadow
- **Requirement 11.3**: Support configurable whale profitability thresholds
- **Requirement 11.4**: Support configurable RPC endpoint URLs
- **Requirement 11.5**: Support configurable database connection parameters
- **Requirement 11.6**: Support configurable Redis connection parameters
- **Requirement 11.7**: Support configurable WebSocket server port and host
- **Requirement 11.8**: Support configurable API server port and host
- **Requirement 11.9**: Fail to start with clear error message when configuration is invalid

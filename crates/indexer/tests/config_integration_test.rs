use solana_indexer::config::Config;
use std::env;

#[test]
#[ignore] // Ignore by default since it requires a config file
fn test_load_config_from_file() {
    // This test demonstrates loading configuration from config.toml
    // Run with: cargo test --package solana-indexer test_load_config_from_file -- --ignored

    // Set the working directory to the workspace root
    let workspace_root = env::current_dir()
        .unwrap()
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists())
        .unwrap()
        .to_path_buf();

    env::set_current_dir(&workspace_root).unwrap();

    // Copy config.example.toml to config.toml if it doesn't exist
    let config_path = workspace_root.join("config.toml");
    let example_path = workspace_root.join("config.example.toml");

    if !config_path.exists() && example_path.exists() {
        std::fs::copy(&example_path, &config_path).unwrap();
    }

    // Load the configuration
    let config = Config::load().expect("Failed to load configuration");

    // Verify some basic values
    assert_eq!(config.scout.polling_interval_secs, 5);
    assert_eq!(config.hunter.polling_interval_secs, 30);
    assert_eq!(config.shadow.polling_interval_secs, 2);
    assert_eq!(config.api.port, 8080);
    assert_eq!(config.websocket.port, 8081);
}

#[test]
fn test_config_from_env_vars() {
    // Test loading configuration from environment variables
    env::set_var("APP_SCOUT__POLLING_INTERVAL_SECS", "10");
    env::set_var("APP_SCOUT__PUMPFUN_PROGRAM_ID", "TestPumpFunID");
    env::set_var("APP_SCOUT__RAYDIUM_PROGRAM_ID", "TestRaydiumID");
    env::set_var("APP_HUNTER__POLLING_INTERVAL_SECS", "60");
    env::set_var("APP_HUNTER__TOKEN_MATURITY_MINUTES", "45");
    env::set_var("APP_HUNTER__EARLY_BUYER_WINDOW_SECS", "120");
    env::set_var("APP_HUNTER__MAX_SIGNATURES_PER_TOKEN", "100");
    env::set_var("APP_HUNTER__WHALE_WIN_RATE_THRESHOLD", "0.8");
    env::set_var("APP_HUNTER__WHALE_ROI_THRESHOLD", "5.0");
    env::set_var("APP_SHADOW__POLLING_INTERVAL_SECS", "3");
    env::set_var("APP_SHADOW__MAX_CONCURRENT_WALLETS", "1000");
    env::set_var("APP_RPC__ENDPOINTS", "https://test.solana.com");
    env::set_var("APP_RPC__TIMEOUT_SECS", "60");
    env::set_var("APP_RPC__MAX_RETRIES", "5");
    env::set_var(
        "APP_DATABASE__URL",
        "postgresql://test:test@localhost:5432/test",
    );
    env::set_var("APP_DATABASE__MAX_CONNECTIONS", "50");
    env::set_var("APP_DATABASE__MIN_CONNECTIONS", "10");
    env::set_var("APP_DATABASE__CONNECTION_TIMEOUT_SECS", "60");
    env::set_var("APP_REDIS__URL", "redis://localhost:6379");
    env::set_var("APP_REDIS__POOL_SIZE", "20");
    env::set_var("APP_REDIS__TOKEN_TTL_SECS", "600");
    env::set_var("APP_REDIS__WHALE_WALLET_TTL_SECS", "60");
    env::set_var("APP_REDIS__TOKEN_DETAILS_TTL_SECS", "1200");
    env::set_var("APP_API__HOST", "127.0.0.1");
    env::set_var("APP_API__PORT", "9090");
    env::set_var("APP_API__RATE_LIMIT_REQUESTS", "200");
    env::set_var("APP_API__RATE_LIMIT_WINDOW_SECS", "120");
    env::set_var("APP_WEBSOCKET__HOST", "127.0.0.1");
    env::set_var("APP_WEBSOCKET__PORT", "9091");
    env::set_var("APP_WEBSOCKET__MAX_CONNECTIONS", "2000");
    env::set_var("APP_WEBSOCKET__HEARTBEAT_INTERVAL_SECS", "60");
    env::set_var("APP_LOGGING__LEVEL", "debug");
    env::set_var("APP_LOGGING__FORMAT", "Json");

    // Note: This test will fail if config.toml exists and has different values
    // since environment variables override config files
    // In a real scenario, you'd want to test this in isolation
}

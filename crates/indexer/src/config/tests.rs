#[cfg(test)]
mod tests {
    use super::super::*;

    fn valid_config() -> Config {
        Config {
            scout: ScoutConfig {
                polling_interval_secs: 5,
                pumpfun_program_id: "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P".to_string(),
                raydium_program_id: "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(),
            },
            hunter: HunterConfig {
                polling_interval_secs: 30,
                token_maturity_minutes: 30,
                early_buyer_window_secs: 60,
                max_signatures_per_token: 50,
                whale_win_rate_threshold: 0.70,
                whale_roi_threshold: 3.0,
            },
            shadow: ShadowConfig {
                polling_interval_secs: 2,
                max_concurrent_wallets: 500,
            },
            rpc: RpcConfig {
                endpoints: vec!["https://api.mainnet-beta.solana.com".to_string()],
                timeout_secs: 30,
                max_retries: 3,
            },
            database: DatabaseConfig {
                url: "postgresql://user:password@localhost:5432/solana_indexer".to_string(),
                max_connections: 20,
                min_connections: 5,
                connection_timeout_secs: 30,
            },
            api: ApiConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                rate_limit_requests: 100,
                rate_limit_window_secs: 60,
            },
            websocket: WebSocketConfig {
                host: "0.0.0.0".to_string(),
                port: 8081,
                max_connections: 1000,
                heartbeat_interval_secs: 30,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: LogFormat::Json,
            },
        }
    }

    #[test]
    fn valid_config_passes_validation() {
        assert!(valid_config().validate().is_ok());
    }

    #[test]
    fn invalid_polling_intervals_fail() {
        let mut config = valid_config();
        config.scout.polling_interval_secs = 0;
        assert!(config.validate().is_err());

        let mut config = valid_config();
        config.hunter.polling_interval_secs = 5;
        assert!(config.validate().is_err());

        let mut config = valid_config();
        config.shadow.polling_interval_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn invalid_connection_settings_fail() {
        let mut config = valid_config();
        config.rpc.endpoints = vec!["ftp://example.com".to_string()];
        assert!(config.validate().is_err());

        let mut config = valid_config();
        config.database.url = "mysql://localhost/db".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn logging_level_is_case_insensitive() {
        for level in ["trace", "debug", "info", "warn", "error", "INFO"] {
            let mut config = valid_config();
            config.logging.level = level.to_string();
            assert!(config.validate().is_ok());
        }
    }
}

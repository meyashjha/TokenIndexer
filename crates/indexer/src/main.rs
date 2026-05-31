#![allow(dead_code)]

use actix_web::{web, App, HttpServer};
use std::sync::Arc;
use tokio::sync::{broadcast, watch};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;

mod config;
mod database;
mod indexers;
mod metrics;
mod models;
mod queue;
mod rpc;

use config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Load configuration
    let config = match Config::load() {
        Ok(config) => {
            // Initialize logging based on config
            init_logging(&config.logging);
            tracing::info!("Configuration loaded successfully");
            config
        }
        Err(e) => {
            // Fallback logging if config fails
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info".into()),
                )
                .with(tracing_subscriber::fmt::layer().json())
                .init();
            tracing::error!(error = %e, "Failed to load configuration");
            return Err(e);
        }
    };

    tracing::info!("Starting Solana Three-Tier Token Indexer");

    // Validate startup connectivity
    tracing::info!("Validating startup configuration...");

    // Initialize RPC client pool
    let rpc_client = Arc::new(rpc::RpcClientPool::new(&config.rpc)?);
    tracing::info!(
        endpoints = config.rpc.endpoints.len(),
        "RPC client pool initialized"
    );

    // Initialize database
    let db_pool = match database::DatabasePool::new(&config.database).await {
        Ok(pool) => {
            tracing::info!("Database pool initialized");
            pool
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to connect to database");
            return Err(e);
        }
    };

    // Run migrations
    if let Err(e) = db_pool.run_migrations().await {
        tracing::warn!(error = %e, "Failed to run migrations (may already be applied)");
    }

    // Initialize repositories
    let token_repo: Arc<dyn database::TokenRepo> =
        Arc::new(database::TokenRepository::new(db_pool.pool().clone()));
    let whale_repo: Arc<dyn database::WhaleWalletRepo> =
        Arc::new(database::WhaleWalletRepository::new(db_pool.pool().clone()));
    let early_buyer_repo: Arc<dyn database::EarlyBuyerRepo> =
        Arc::new(database::EarlyBuyerRepository::new(db_pool.pool().clone()));
    let alert_repo: Arc<dyn database::WhaleAlertRepo> =
        Arc::new(database::WhaleAlertRepository::new(db_pool.pool().clone()));

    // Initialize message queue
    let message_queue: Arc<dyn queue::MessageQueue> = Arc::new(queue::InMemoryQueue::new());
    tracing::info!("Message queue initialized");

    // Initialize metrics
    let app_metrics = Arc::new(metrics::Metrics::new()?);
    tracing::info!("Metrics initialized");

    // Create shutdown signal
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Create WebSocket broadcast channel
    let (ws_sender, _) = broadcast::channel::<String>(1000);
    let ws_sender_clone = ws_sender.clone();

    // Spawn Alert Dispatcher
    let dispatcher_queue = message_queue.clone();
    let dispatcher_shutdown = shutdown_rx.clone();
    let dispatcher_sender = ws_sender.clone();
    tokio::spawn(async move {
        let dispatcher = api::dispatcher::AlertDispatcher::new(dispatcher_queue, dispatcher_sender);
        if let Err(e) = dispatcher.run(dispatcher_shutdown).await {
            tracing::error!(error = %e, "Alert Dispatcher error");
        }
    });
    tracing::info!("Alert Dispatcher started");

    // Spawn Scout indexer
    let scout_config = config.scout.clone();
    let scout_rpc = rpc_client.clone();
    let scout_repo = token_repo.clone();
    let scout_queue = message_queue.clone();
    let scout_shutdown = shutdown_rx.clone();
    tokio::spawn(async move {
        let scout = indexers::Scout::new(scout_config, scout_rpc, scout_repo, scout_queue);
        if let Err(e) = scout.run(scout_shutdown).await {
            tracing::error!(error = %e, "Scout error");
        }
    });
    tracing::info!("Scout started");

    // Spawn Hunter indexer
    let hunter_config = config.hunter.clone();
    let hunter_rpc = rpc_client.clone();
    let hunter_token_repo = token_repo.clone();
    let hunter_whale_repo = whale_repo.clone();
    let hunter_early_buyer_repo = early_buyer_repo.clone();
    let hunter_queue = message_queue.clone();
    let hunter_shutdown = shutdown_rx.clone();
    tokio::spawn(async move {
        let hunter = indexers::Hunter::new(
            hunter_config,
            hunter_rpc,
            hunter_token_repo,
            hunter_whale_repo,
            hunter_early_buyer_repo,
            hunter_queue,
        );
        if let Err(e) = hunter.run(hunter_shutdown).await {
            tracing::error!(error = %e, "Hunter error");
        }
    });
    tracing::info!("Hunter started");

    // Spawn Shadow indexer
    let shadow_config = config.shadow.clone();
    let shadow_rpc = rpc_client.clone();
    let shadow_whale_repo = whale_repo.clone();
    let shadow_alert_repo = alert_repo.clone();
    let shadow_queue = message_queue.clone();
    let shadow_shutdown = shutdown_rx.clone();
    tokio::spawn(async move {
        let shadow = indexers::Shadow::new(
            shadow_config,
            shadow_rpc,
            shadow_whale_repo,
            shadow_alert_repo,
            shadow_queue,
        );
        if let Err(e) = shadow.run(shadow_shutdown).await {
            tracing::error!(error = %e, "Shadow error");
        }
    });
    tracing::info!("Shadow started");

    // Start API server
    let api_host = config.api.host.clone();
    let api_port = config.api.port;
    let api_config = config.api.clone();

    let api_state = web::Data::new(api::AppState {
        token_repo: token_repo.clone(),
        whale_repo: whale_repo.clone(),
        alert_repo: alert_repo.clone(),
        early_buyer_repo: early_buyer_repo.clone(),
        config: api_config,
    });

    let metrics_data = web::Data::new(app_metrics.clone());
    let ws_data = web::Data::new(ws_sender_clone);

    tracing::info!(
        host = %api_host,
        port = api_port,
        "Starting API server"
    );

    let server = HttpServer::new(move || {
        App::new()
            .app_data(api_state.clone())
            .app_data(metrics_data.clone())
            .app_data(ws_data.clone())
            .configure(api::configure_routes)
            .route("/metrics", web::get().to(metrics::metrics_handler))
            .route("/ws", web::get().to(api::websocket::ws_handler))
    })
    .bind(format!("{}:{}", api_host, api_port))?
    .run();

    let server_handle = server.handle();

    // Spawn the server
    tokio::spawn(server);

    tracing::info!("Indexer started successfully - all components running");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutdown signal received");

    // Signal all components to stop
    let _ = shutdown_tx.send(true);

    // Stop the API server
    server_handle.stop(true).await;

    tracing::info!("Shutdown complete");
    Ok(())
}

/// Initialize structured logging based on config
fn init_logging(config: &config::LoggingConfig) {
    let level = config.level.clone();

    let registry = tracing_subscriber::registry().with(
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| level.into()),
    );

    match config.format {
        config::LogFormat::Json => {
            registry
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        }
        config::LogFormat::Pretty => {
            registry
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
    }
}

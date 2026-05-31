use crate::config::DatabaseConfig;
use crate::models::{
    EarlyBuyerTransaction, LaunchpadSource, TokenMetadata, TokenTransaction, WalletMetrics,
    WalletType, WhaleAlert, WhaleWallet,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::str::FromStr;
use std::time::Duration;

pub struct DatabasePool {
    pool: PgPool,
}

impl DatabasePool {
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connection_timeout_secs))
            .connect(&config.url)
            .await
            .context("failed to connect to PostgreSQL")?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("failed to run database migrations")
    }
}

#[async_trait]
pub trait TokenRepo: Send + Sync {
    async fn insert(&self, token: &TokenMetadata) -> Result<()>;
    async fn get_by_mint(&self, mint_address: &str) -> Result<Option<TokenMetadata>>;
    async fn list_recent(&self, limit: i64) -> Result<Vec<TokenMetadata>>;
    async fn list_unanalyzed_mature(
        &self,
        maturity_minutes: u64,
        limit: i64,
    ) -> Result<Vec<TokenMetadata>>;
    async fn list_active_tokens(&self, active_days: i64, limit: i64) -> Result<Vec<TokenMetadata>>;
    async fn mark_analyzed(&self, mint_address: &str) -> Result<()>;
    async fn update_indexed_status(
        &self,
        mint_address: &str,
        last_signature: Option<&str>,
    ) -> Result<()>;
}

pub struct TokenRepository {
    pool: PgPool,
}

impl TokenRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct TokenRow {
    mint_address: String,
    creation_timestamp: DateTime<Utc>,
    launchpad_source: String,
    slot_number: i64,
    analyzed: bool,
    last_indexed_signature: Option<String>,
    last_indexed_at: Option<DateTime<Utc>>,
}

impl TryFrom<TokenRow> for TokenMetadata {
    type Error = anyhow::Error;

    fn try_from(row: TokenRow) -> Result<Self> {
        Ok(Self {
            mint_address: row.mint_address,
            creation_timestamp: row.creation_timestamp,
            launchpad_source: LaunchpadSource::from_str(&row.launchpad_source)?,
            slot_number: row.slot_number.max(0) as u64,
            analyzed: row.analyzed,
            last_indexed_signature: row.last_indexed_signature,
            last_indexed_at: row.last_indexed_at,
        })
    }
}

#[async_trait]
impl TokenRepo for TokenRepository {
    async fn insert(&self, token: &TokenMetadata) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO tokens (
                mint_address, creation_timestamp, launchpad_source, slot_number,
                analyzed, last_indexed_signature, last_indexed_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (mint_address) DO UPDATE SET
                creation_timestamp = EXCLUDED.creation_timestamp,
                launchpad_source = EXCLUDED.launchpad_source,
                slot_number = EXCLUDED.slot_number,
                updated_at = NOW()
            "#,
        )
        .bind(&token.mint_address)
        .bind(token.creation_timestamp)
        .bind(token.launchpad_source.to_string())
        .bind(token.slot_number as i64)
        .bind(token.analyzed)
        .bind(&token.last_indexed_signature)
        .bind(token.last_indexed_at)
        .execute(&self.pool)
        .await
        .context("failed to insert token")?;

        Ok(())
    }

    async fn get_by_mint(&self, mint_address: &str) -> Result<Option<TokenMetadata>> {
        let row = sqlx::query_as::<_, TokenRow>(
            r#"
            SELECT mint_address, creation_timestamp, launchpad_source, slot_number,
                   analyzed, last_indexed_signature, last_indexed_at
            FROM tokens
            WHERE mint_address = $1
            "#,
        )
        .bind(mint_address)
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch token by mint")?;

        row.map(TokenMetadata::try_from).transpose()
    }

    async fn list_recent(&self, limit: i64) -> Result<Vec<TokenMetadata>> {
        fetch_tokens(
            &self.pool,
            r#"
            SELECT mint_address, creation_timestamp, launchpad_source, slot_number,
                   analyzed, last_indexed_signature, last_indexed_at
            FROM tokens
            ORDER BY creation_timestamp DESC
            LIMIT $1
            "#,
            vec![limit],
        )
        .await
    }

    async fn list_unanalyzed_mature(
        &self,
        maturity_minutes: u64,
        limit: i64,
    ) -> Result<Vec<TokenMetadata>> {
        let rows = sqlx::query_as::<_, TokenRow>(
            r#"
            SELECT mint_address, creation_timestamp, launchpad_source, slot_number,
                   analyzed, last_indexed_signature, last_indexed_at
            FROM tokens
            WHERE analyzed = FALSE
              AND creation_timestamp <= NOW() - ($1::BIGINT * INTERVAL '1 minute')
            ORDER BY creation_timestamp ASC
            LIMIT $2
            "#,
        )
        .bind(maturity_minutes as i64)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to list mature tokens")?;

        rows.into_iter().map(TokenMetadata::try_from).collect()
    }

    async fn list_active_tokens(&self, active_days: i64, limit: i64) -> Result<Vec<TokenMetadata>> {
        let rows = sqlx::query_as::<_, TokenRow>(
            r#"
            SELECT mint_address, creation_timestamp, launchpad_source, slot_number,
                   analyzed, last_indexed_signature, last_indexed_at
            FROM tokens
            WHERE creation_timestamp >= NOW() - ($1::BIGINT * INTERVAL '1 day')
            ORDER BY last_indexed_at ASC NULLS FIRST, creation_timestamp DESC
            LIMIT $2
            "#,
        )
        .bind(active_days)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to list active tokens")?;

        rows.into_iter().map(TokenMetadata::try_from).collect()
    }

    async fn mark_analyzed(&self, mint_address: &str) -> Result<()> {
        sqlx::query(
            "UPDATE tokens SET analyzed = TRUE, updated_at = NOW() WHERE mint_address = $1",
        )
        .bind(mint_address)
        .execute(&self.pool)
        .await
        .context("failed to mark token analyzed")?;
        Ok(())
    }

    async fn update_indexed_status(
        &self,
        mint_address: &str,
        last_signature: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE tokens
            SET last_indexed_signature = COALESCE($2, last_indexed_signature),
                last_indexed_at = NOW(),
                updated_at = NOW()
            WHERE mint_address = $1
            "#,
        )
        .bind(mint_address)
        .bind(last_signature)
        .execute(&self.pool)
        .await
        .context("failed to update token indexing status")?;
        Ok(())
    }
}

async fn fetch_tokens(pool: &PgPool, sql: &str, binds: Vec<i64>) -> Result<Vec<TokenMetadata>> {
    let mut query = sqlx::query_as::<_, TokenRow>(sql);
    for bind in binds {
        query = query.bind(bind);
    }
    let rows = query
        .fetch_all(pool)
        .await
        .context("failed to fetch tokens")?;
    rows.into_iter().map(TokenMetadata::try_from).collect()
}

#[async_trait]
pub trait WhaleWalletRepo: Send + Sync {
    async fn upsert(&self, wallet: &WhaleWallet) -> Result<()>;
    async fn get_by_address(&self, wallet_address: &str) -> Result<Option<WhaleWallet>>;
    async fn get_all(&self) -> Result<Vec<WhaleWallet>>;
    async fn list_paginated(&self, offset: i64, limit: i64) -> Result<(Vec<WhaleWallet>, i64)>;
}

pub struct WhaleWalletRepository {
    pool: PgPool,
}

impl WhaleWalletRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct WhaleWalletRow {
    wallet_address: String,
    win_rate: f64,
    average_roi: f64,
    wallet_type: String,
    total_trades_analyzed: i32,
    last_updated: DateTime<Utc>,
}

impl TryFrom<WhaleWalletRow> for WhaleWallet {
    type Error = anyhow::Error;

    fn try_from(row: WhaleWalletRow) -> Result<Self> {
        Ok(Self {
            wallet_address: row.wallet_address,
            win_rate: row.win_rate,
            average_roi: row.average_roi,
            wallet_type: WalletType::from_str(&row.wallet_type)?,
            last_updated: row.last_updated,
            total_trades_analyzed: row.total_trades_analyzed.max(0) as u32,
        })
    }
}

#[async_trait]
impl WhaleWalletRepo for WhaleWalletRepository {
    async fn upsert(&self, wallet: &WhaleWallet) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO whale_wallets (
                wallet_address, win_rate, average_roi, wallet_type,
                total_trades_analyzed, last_updated
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (wallet_address) DO UPDATE SET
                win_rate = EXCLUDED.win_rate,
                average_roi = EXCLUDED.average_roi,
                wallet_type = EXCLUDED.wallet_type,
                total_trades_analyzed = EXCLUDED.total_trades_analyzed,
                last_updated = EXCLUDED.last_updated
            "#,
        )
        .bind(&wallet.wallet_address)
        .bind(wallet.win_rate)
        .bind(wallet.average_roi)
        .bind(wallet.wallet_type.to_string())
        .bind(wallet.total_trades_analyzed as i32)
        .bind(wallet.last_updated)
        .execute(&self.pool)
        .await
        .context("failed to upsert whale wallet")?;
        Ok(())
    }

    async fn get_by_address(&self, wallet_address: &str) -> Result<Option<WhaleWallet>> {
        let row = sqlx::query_as::<_, WhaleWalletRow>(
            r#"
            SELECT wallet_address, win_rate, average_roi, wallet_type,
                   total_trades_analyzed, last_updated
            FROM whale_wallets
            WHERE wallet_address = $1
            "#,
        )
        .bind(wallet_address)
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch whale wallet")?;

        row.map(WhaleWallet::try_from).transpose()
    }

    async fn get_all(&self) -> Result<Vec<WhaleWallet>> {
        let rows = sqlx::query_as::<_, WhaleWalletRow>(
            r#"
            SELECT wallet_address, win_rate, average_roi, wallet_type,
                   total_trades_analyzed, last_updated
            FROM whale_wallets
            ORDER BY win_rate DESC, average_roi DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch whale wallets")?;

        rows.into_iter().map(WhaleWallet::try_from).collect()
    }

    async fn list_paginated(&self, offset: i64, limit: i64) -> Result<(Vec<WhaleWallet>, i64)> {
        let rows = sqlx::query_as::<_, WhaleWalletRow>(
            r#"
            SELECT wallet_address, win_rate, average_roi, wallet_type,
                   total_trades_analyzed, last_updated
            FROM whale_wallets
            ORDER BY win_rate DESC, average_roi DESC
            OFFSET $1
            LIMIT $2
            "#,
        )
        .bind(offset)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to list whale wallets")?;

        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM whale_wallets")
            .fetch_one(&self.pool)
            .await
            .context("failed to count whale wallets")?;

        Ok((
            rows.into_iter()
                .map(WhaleWallet::try_from)
                .collect::<Result<Vec<_>>>()?,
            total,
        ))
    }
}

#[async_trait]
pub trait EarlyBuyerRepo: Send + Sync {
    async fn insert(&self, transaction: &EarlyBuyerTransaction) -> Result<()>;
    async fn insert_token_transaction(&self, transaction: &TokenTransaction) -> Result<()>;
    async fn insert_batch(&self, transactions: &[EarlyBuyerTransaction]) -> Result<()>;
    async fn insert_token_transactions_batch(&self, transactions: &[TokenTransaction]) -> Result<()>;
    async fn list_transactions_for_token(
        &self,
        token_mint: &str,
        limit: i64,
    ) -> Result<Vec<TokenTransaction>>;
}

#[derive(sqlx::FromRow)]
struct TokenTransactionRow {
    signature: String,
    token_mint: String,
    buyer_address: String,
    amount: f64,
    slot_number: i64,
    timestamp: DateTime<Utc>,
}

impl From<TokenTransactionRow> for TokenTransaction {
    fn from(row: TokenTransactionRow) -> Self {
        Self {
            signature: row.signature,
            token_mint: row.token_mint,
            buyer_address: row.buyer_address,
            amount: row.amount,
            slot_number: row.slot_number.max(0) as u64,
            timestamp: row.timestamp,
        }
    }
}

pub struct EarlyBuyerRepository {
    pool: PgPool,
}

impl EarlyBuyerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EarlyBuyerRepo for EarlyBuyerRepository {
    async fn insert(&self, transaction: &EarlyBuyerTransaction) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO early_buyer_transactions (
                signature, wallet_address, token_mint, purchase_slot,
                slot_delta, amount, timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (signature) DO NOTHING
            "#,
        )
        .bind(&transaction.signature)
        .bind(&transaction.wallet_address)
        .bind(&transaction.token_mint)
        .bind(transaction.purchase_slot as i64)
        .bind(transaction.slot_delta)
        .bind(transaction.amount)
        .bind(transaction.timestamp)
        .execute(&self.pool)
        .await
        .context("failed to insert early buyer transaction")?;
        Ok(())
    }

    async fn insert_token_transaction(&self, transaction: &TokenTransaction) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO token_transactions (
                signature, token_mint, buyer_address, amount, slot_number, timestamp
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (signature) DO UPDATE SET
                token_mint = EXCLUDED.token_mint,
                buyer_address = EXCLUDED.buyer_address,
                amount = EXCLUDED.amount,
                slot_number = EXCLUDED.slot_number,
                timestamp = EXCLUDED.timestamp
            "#,
        )
        .bind(&transaction.signature)
        .bind(&transaction.token_mint)
        .bind(&transaction.buyer_address)
        .bind(transaction.amount)
        .bind(transaction.slot_number as i64)
        .bind(transaction.timestamp)
        .execute(&self.pool)
        .await
        .context("failed to insert token transaction")?;
        Ok(())
    }

    async fn insert_batch(&self, transactions: &[EarlyBuyerTransaction]) -> Result<()> {
        if transactions.is_empty() {
            return Ok(());
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO early_buyer_transactions (signature, wallet_address, token_mint, purchase_slot, slot_delta, amount, timestamp) "
        );

        query_builder.push_values(transactions, |mut b, tx| {
            b.push_bind(&tx.signature);
            b.push_bind(&tx.wallet_address);
            b.push_bind(&tx.token_mint);
            b.push_bind(tx.purchase_slot as i64);
            b.push_bind(tx.slot_delta);
            b.push_bind(tx.amount);
            b.push_bind(tx.timestamp);
        });

        query_builder.push(" ON CONFLICT (signature) DO NOTHING");

        let query = query_builder.build();
        query.execute(&self.pool)
            .await
            .context("failed to insert batch early buyer transactions")?;

        Ok(())
    }

    async fn insert_token_transactions_batch(&self, transactions: &[TokenTransaction]) -> Result<()> {
        if transactions.is_empty() {
            return Ok(());
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO token_transactions (signature, token_mint, buyer_address, amount, slot_number, timestamp) "
        );

        query_builder.push_values(transactions, |mut b, tx| {
            b.push_bind(&tx.signature);
            b.push_bind(&tx.token_mint);
            b.push_bind(&tx.buyer_address);
            b.push_bind(tx.amount);
            b.push_bind(tx.slot_number as i64);
            b.push_bind(tx.timestamp);
        });

        query_builder.push(" ON CONFLICT (signature) DO NOTHING");

        let query = query_builder.build();
        query.execute(&self.pool)
            .await
            .context("failed to insert batch token transactions")?;

        Ok(())
    }

    async fn list_transactions_for_token(
        &self,
        token_mint: &str,
        limit: i64,
    ) -> Result<Vec<TokenTransaction>> {
        let rows = sqlx::query_as::<_, TokenTransactionRow>(
            r#"
            SELECT signature, token_mint, buyer_address, amount, slot_number, timestamp
            FROM token_transactions
            WHERE token_mint = $1
            ORDER BY timestamp DESC, slot_number DESC
            LIMIT $2
            "#
        )
        .bind(token_mint)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to list token transactions")?;

        Ok(rows.into_iter().map(TokenTransaction::from).collect())
    }
}

#[async_trait]
pub trait WhaleAlertRepo: Send + Sync {
    async fn insert(&self, alert: &WhaleAlert) -> Result<()>;
    async fn get_by_time_range(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<WhaleAlert>>;
}

pub struct WhaleAlertRepository {
    pool: PgPool,
}

impl WhaleAlertRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct WhaleAlertRow {
    id: uuid::Uuid,
    timestamp: DateTime<Utc>,
    wallet_address: String,
    token_address: String,
    purchase_amount: f64,
    wallet_win_rate: f64,
    wallet_average_roi: f64,
    wallet_type: String,
    transaction_signature: Option<String>,
    slot_number: Option<i64>,
}

impl TryFrom<WhaleAlertRow> for WhaleAlert {
    type Error = anyhow::Error;

    fn try_from(row: WhaleAlertRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            timestamp: row.timestamp,
            wallet_address: row.wallet_address,
            token_address: row.token_address,
            purchase_amount: row.purchase_amount,
            wallet_metrics: WalletMetrics {
                win_rate: row.wallet_win_rate,
                average_roi: row.wallet_average_roi,
                wallet_type: WalletType::from_str(&row.wallet_type)?,
            },
            transaction_signature: row.transaction_signature,
            slot_number: row.slot_number.map(|slot| slot.max(0) as u64),
        })
    }
}

#[async_trait]
impl WhaleAlertRepo for WhaleAlertRepository {
    async fn insert(&self, alert: &WhaleAlert) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO whale_alerts (
                id, timestamp, wallet_address, token_address, purchase_amount,
                wallet_win_rate, wallet_average_roi, wallet_type,
                transaction_signature, slot_number
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(alert.id)
        .bind(alert.timestamp)
        .bind(&alert.wallet_address)
        .bind(&alert.token_address)
        .bind(alert.purchase_amount)
        .bind(alert.wallet_metrics.win_rate)
        .bind(alert.wallet_metrics.average_roi)
        .bind(alert.wallet_metrics.wallet_type.to_string())
        .bind(&alert.transaction_signature)
        .bind(alert.slot_number.map(|slot| slot as i64))
        .execute(&self.pool)
        .await
        .context("failed to insert whale alert")?;
        Ok(())
    }

    async fn get_by_time_range(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<WhaleAlert>> {
        let rows = sqlx::query_as::<_, WhaleAlertRow>(
            r#"
            SELECT id, timestamp, wallet_address, token_address, purchase_amount,
                   wallet_win_rate, wallet_average_roi, wallet_type,
                   transaction_signature, slot_number
            FROM whale_alerts
            WHERE timestamp >= $1 AND timestamp <= $2
            ORDER BY timestamp DESC
            "#,
        )
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch whale alerts")?;

        rows.into_iter().map(WhaleAlert::try_from).collect()
    }
}

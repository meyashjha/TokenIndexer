-- Migration: Create initial tables for Solana Three-Tier Token Indexer
-- Version: 001
-- Requirements: 6.1, 6.2, 6.3, 6.4, 6.7

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Tokens table: stores discovered token metadata
CREATE TABLE IF NOT EXISTS tokens (
    id SERIAL PRIMARY KEY,
    mint_address VARCHAR(64) NOT NULL UNIQUE,
    creation_timestamp TIMESTAMPTZ NOT NULL,
    launchpad_source VARCHAR(20) NOT NULL,
    slot_number BIGINT NOT NULL,
    analyzed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for tokens table
CREATE INDEX idx_tokens_creation_timestamp ON tokens(creation_timestamp);
CREATE INDEX idx_tokens_analyzed ON tokens(analyzed);
CREATE INDEX idx_tokens_slot_number ON tokens(slot_number);

-- Whale wallets table: stores identified whale wallets with profitability metrics
CREATE TABLE IF NOT EXISTS whale_wallets (
    id SERIAL PRIMARY KEY,
    wallet_address VARCHAR(64) NOT NULL UNIQUE,
    win_rate DOUBLE PRECISION NOT NULL,
    average_roi DOUBLE PRECISION NOT NULL,
    wallet_type VARCHAR(30) NOT NULL,
    total_trades_analyzed INTEGER NOT NULL DEFAULT 0,
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for whale_wallets table
CREATE INDEX idx_whale_wallets_win_rate ON whale_wallets(win_rate);
CREATE INDEX idx_whale_wallets_average_roi ON whale_wallets(average_roi);

-- Early buyer transactions table: stores early buyer purchase records
CREATE TABLE IF NOT EXISTS early_buyer_transactions (
    id SERIAL PRIMARY KEY,
    wallet_address VARCHAR(64) NOT NULL,
    token_mint VARCHAR(64) NOT NULL,
    purchase_slot BIGINT NOT NULL,
    slot_delta BIGINT NOT NULL,
    amount DOUBLE PRECISION NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for early_buyer_transactions table
CREATE INDEX idx_early_buyers_token_mint ON early_buyer_transactions(token_mint);
CREATE INDEX idx_early_buyers_wallet_address ON early_buyer_transactions(wallet_address);
CREATE INDEX idx_early_buyers_timestamp ON early_buyer_transactions(timestamp);

-- Whale alerts table: stores whale purchase alert history
CREATE TABLE IF NOT EXISTS whale_alerts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    timestamp TIMESTAMPTZ NOT NULL,
    wallet_address VARCHAR(64) NOT NULL,
    token_address VARCHAR(64) NOT NULL,
    purchase_amount DOUBLE PRECISION NOT NULL,
    wallet_win_rate DOUBLE PRECISION NOT NULL,
    wallet_average_roi DOUBLE PRECISION NOT NULL,
    wallet_type VARCHAR(30) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for whale_alerts table
CREATE INDEX idx_whale_alerts_timestamp ON whale_alerts(timestamp);
CREATE INDEX idx_whale_alerts_wallet_address ON whale_alerts(wallet_address);
CREATE INDEX idx_whale_alerts_token_address ON whale_alerts(token_address);

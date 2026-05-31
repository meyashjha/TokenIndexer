-- Migration: Store parsed token transactions and transaction identifiers
-- Version: 003

ALTER TABLE early_buyer_transactions
ADD COLUMN IF NOT EXISTS signature VARCHAR(128);

UPDATE early_buyer_transactions
SET signature = CONCAT('legacy:', id)
WHERE signature IS NULL;

ALTER TABLE early_buyer_transactions
ALTER COLUMN signature SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_early_buyers_signature_unique
ON early_buyer_transactions(signature);

CREATE TABLE IF NOT EXISTS token_transactions (
    id SERIAL PRIMARY KEY,
    signature VARCHAR(128) NOT NULL UNIQUE,
    token_mint VARCHAR(64) NOT NULL,
    buyer_address VARCHAR(64) NOT NULL,
    amount DOUBLE PRECISION NOT NULL,
    slot_number BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_token_transactions_token_mint
ON token_transactions(token_mint);
CREATE INDEX IF NOT EXISTS idx_token_transactions_buyer_address
ON token_transactions(buyer_address);
CREATE INDEX IF NOT EXISTS idx_token_transactions_slot_number
ON token_transactions(slot_number);
CREATE INDEX IF NOT EXISTS idx_token_transactions_timestamp
ON token_transactions(timestamp);

ALTER TABLE whale_alerts
ADD COLUMN IF NOT EXISTS transaction_signature VARCHAR(128),
ADD COLUMN IF NOT EXISTS slot_number BIGINT;

CREATE INDEX IF NOT EXISTS idx_whale_alerts_transaction_signature
ON whale_alerts(transaction_signature);

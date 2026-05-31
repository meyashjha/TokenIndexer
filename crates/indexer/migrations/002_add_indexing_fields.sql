-- Migration: Add last_indexed_signature and last_indexed_at tracking fields to tokens
-- Version: 002

ALTER TABLE tokens 
ADD COLUMN IF NOT EXISTS last_indexed_signature VARCHAR(128),
ADD COLUMN IF NOT EXISTS last_indexed_at TIMESTAMPTZ;

-- Index to optimize retrieval of tokens ordered by last_indexed_at
CREATE INDEX IF NOT EXISTS idx_tokens_last_indexed ON tokens(last_indexed_at);

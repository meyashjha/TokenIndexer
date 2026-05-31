-- Migration: Add performance composite and partial indexes
-- Version: 004

-- Composite index to cover API paginated whale ranking query
CREATE INDEX IF NOT EXISTS idx_whale_wallets_ranking 
ON whale_wallets(win_rate DESC, average_roi DESC);

-- Partial index to cover mature unanalyzed token polling query
CREATE INDEX IF NOT EXISTS idx_tokens_unanalyzed 
ON tokens(creation_timestamp ASC) 
WHERE analyzed = FALSE;

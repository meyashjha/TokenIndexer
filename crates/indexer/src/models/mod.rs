use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LaunchpadSource {
    PumpFun,
    Raydium,
    Unknown,
}

impl fmt::Display for LaunchpadSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PumpFun => "pumpfun",
            Self::Raydium => "raydium",
            Self::Unknown => "unknown",
        })
    }
}

impl FromStr for LaunchpadSource {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value.to_ascii_lowercase().as_str() {
            "pumpfun" | "pump.fun" => Self::PumpFun,
            "raydium" => Self::Raydium,
            _ => Self::Unknown,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenMetadata {
    pub mint_address: String,
    pub creation_timestamp: DateTime<Utc>,
    pub launchpad_source: LaunchpadSource,
    pub slot_number: u64,
    pub analyzed: bool,
    pub last_indexed_signature: Option<String>,
    pub last_indexed_at: Option<DateTime<Utc>>,
}

impl TokenMetadata {
    pub fn new(
        mint_address: String,
        creation_timestamp: DateTime<Utc>,
        launchpad_source: LaunchpadSource,
        slot_number: u64,
    ) -> Self {
        Self {
            mint_address,
            creation_timestamp,
            launchpad_source,
            slot_number,
            analyzed: false,
            last_indexed_signature: None,
            last_indexed_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WalletType {
    EstablishedSniper,
    ConsistentTrader,
    HighRoiWallet,
}

impl fmt::Display for WalletType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EstablishedSniper => "established_sniper",
            Self::ConsistentTrader => "consistent_trader",
            Self::HighRoiWallet => "high_roi_wallet",
        })
    }
}

impl FromStr for WalletType {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value.to_ascii_lowercase().as_str() {
            "consistent_trader" => Self::ConsistentTrader,
            "high_roi_wallet" => Self::HighRoiWallet,
            _ => Self::EstablishedSniper,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalletMetrics {
    pub win_rate: f64,
    pub average_roi: f64,
    pub wallet_type: WalletType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WhaleWallet {
    pub wallet_address: String,
    pub win_rate: f64,
    pub average_roi: f64,
    pub wallet_type: WalletType,
    pub last_updated: DateTime<Utc>,
    pub total_trades_analyzed: u32,
}

impl WhaleWallet {
    pub fn new(
        wallet_address: String,
        win_rate: f64,
        average_roi: f64,
        wallet_type: WalletType,
        total_trades_analyzed: u32,
    ) -> Self {
        Self {
            wallet_address,
            win_rate,
            average_roi,
            wallet_type,
            last_updated: Utc::now(),
            total_trades_analyzed,
        }
    }

    pub fn metrics(&self) -> WalletMetrics {
        WalletMetrics {
            win_rate: self.win_rate,
            average_roi: self.average_roi,
            wallet_type: self.wallet_type.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EarlyBuyerTransaction {
    pub signature: String,
    pub wallet_address: String,
    pub token_mint: String,
    pub purchase_slot: u64,
    pub slot_delta: i64,
    pub amount: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenTransaction {
    pub signature: String,
    pub token_mint: String,
    pub buyer_address: String,
    pub amount: f64,
    pub slot_number: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WhaleAlert {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub wallet_address: String,
    pub token_address: String,
    pub purchase_amount: f64,
    pub wallet_metrics: WalletMetrics,
    pub transaction_signature: Option<String>,
    pub slot_number: Option<u64>,
}

impl WhaleAlert {
    pub fn new(
        wallet_address: String,
        token_address: String,
        purchase_amount: f64,
        wallet_metrics: WalletMetrics,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            wallet_address,
            token_address,
            purchase_amount,
            wallet_metrics,
            transaction_signature: None,
            slot_number: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trade {
    pub token_mint: String,
    pub entry_price: f64,
    pub exit_price: f64,
    pub amount: f64,
    pub entry_timestamp: DateTime<Utc>,
    pub exit_timestamp: DateTime<Utc>,
}

impl Trade {
    pub fn roi(&self) -> f64 {
        (self.exit_price - self.entry_price) / self.entry_price
    }

    pub fn is_profitable(&self) -> bool {
        self.exit_price > self.entry_price
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginationParams {
    pub page: u32,
    pub page_size: u32,
}

impl PaginationParams {
    pub fn new(page: u32, page_size: u32) -> Self {
        Self {
            page: page.max(1),
            page_size: page_size.clamp(1, 100),
        }
    }

    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.page_size
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, total: u64, params: &PaginationParams) -> Self {
        let total_pages = if total == 0 {
            0
        } else {
            total.div_ceil(params.page_size as u64) as u32
        };

        Self {
            data,
            total,
            page: params.page,
            page_size: params.page_size,
            total_pages,
        }
    }
}

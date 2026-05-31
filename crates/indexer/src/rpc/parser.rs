use crate::models::LaunchpadSource;
use anyhow::Result;
use serde_json::Value;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedTokenMint {
    pub mint_address: String,
    pub slot: u64,
    pub launchpad_source: LaunchpadSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPurchase {
    pub token_address: String,
    pub buyer_address: String,
    pub amount: f64,
    pub slot: u64,
}

pub fn parse_token_mint(
    transaction: &EncodedConfirmedTransactionWithStatusMeta,
    pumpfun_program_id: &str,
    raydium_program_id: &str,
) -> Result<Option<ParsedTokenMint>> {
    let value = serde_json::to_value(transaction)?;
    let text = value.to_string();
    let source = if text.contains(pumpfun_program_id) {
        LaunchpadSource::PumpFun
    } else if text.contains(raydium_program_id) {
        LaunchpadSource::Raydium
    } else {
        return Ok(None);
    };

    let slot = value
        .get("slot")
        .and_then(Value::as_u64)
        .unwrap_or_default();

    for instruction in collect_instructions(&value) {
        if let Some(mint) = instruction
            .pointer("/parsed/info/mint")
            .and_then(Value::as_str)
            .or_else(|| {
                instruction
                    .pointer("/parsed/info/account")
                    .and_then(Value::as_str)
            })
        {
            if looks_like_pubkey(mint) {
                return Ok(Some(ParsedTokenMint {
                    mint_address: mint.to_string(),
                    slot,
                    launchpad_source: source,
                }));
            }
        }
    }

    if let Some(mint) = value
        .pointer("/meta/postTokenBalances")
        .and_then(Value::as_array)
        .and_then(|balances| {
            balances
                .iter()
                .find_map(|b| b.get("mint").and_then(Value::as_str))
        })
    {
        return Ok(Some(ParsedTokenMint {
            mint_address: mint.to_string(),
            slot,
            launchpad_source: source,
        }));
    }

    Ok(None)
}

pub fn extract_purchase_data(
    transaction: &EncodedConfirmedTransactionWithStatusMeta,
) -> Result<Option<ParsedPurchase>> {
    let value = serde_json::to_value(transaction)?;
    let slot = value
        .get("slot")
        .and_then(Value::as_u64)
        .unwrap_or_default();

    if let Some(purchase) = purchase_from_token_balances(&value, slot) {
        return Ok(Some(purchase));
    }

    for instruction in collect_instructions(&value) {
        let parsed_type = instruction
            .pointer("/parsed/type")
            .and_then(Value::as_str)
            .unwrap_or_default();

        if !matches!(
            parsed_type,
            "transfer" | "transferChecked" | "mintTo" | "mintToChecked"
        ) {
            continue;
        }

        let Some(info) = instruction.pointer("/parsed/info") else {
            continue;
        };

        let token_address = info
            .get("mint")
            .and_then(Value::as_str)
            .or_else(|| info.get("token").and_then(Value::as_str));
        let buyer_address = info
            .get("destination")
            .and_then(Value::as_str)
            .or_else(|| info.get("owner").and_then(Value::as_str))
            .or_else(|| info.get("authority").and_then(Value::as_str));
        let amount = token_amount(info);

        if let (Some(token_address), Some(buyer_address), Some(amount)) =
            (token_address, buyer_address, amount)
        {
            if amount > 0.0 && looks_like_pubkey(token_address) && looks_like_pubkey(buyer_address)
            {
                return Ok(Some(ParsedPurchase {
                    token_address: token_address.to_string(),
                    buyer_address: buyer_address.to_string(),
                    amount,
                    slot,
                }));
            }
        }
    }

    Ok(None)
}

fn purchase_from_token_balances(value: &Value, slot: u64) -> Option<ParsedPurchase> {
    let pre = value
        .pointer("/meta/preTokenBalances")
        .and_then(Value::as_array)?;
    let post = value
        .pointer("/meta/postTokenBalances")
        .and_then(Value::as_array)?;

    for post_balance in post {
        let account_index = post_balance.get("accountIndex").and_then(Value::as_u64)?;
        let mint = post_balance.get("mint").and_then(Value::as_str)?;
        let owner = post_balance.get("owner").and_then(Value::as_str)?;
        let post_amount = ui_amount(post_balance)?;
        let pre_amount = pre
            .iter()
            .find(|entry| entry.get("accountIndex").and_then(Value::as_u64) == Some(account_index))
            .and_then(ui_amount)
            .unwrap_or(0.0);
        let delta = post_amount - pre_amount;

        if delta > 0.0 && looks_like_pubkey(mint) && looks_like_pubkey(owner) {
            return Some(ParsedPurchase {
                token_address: mint.to_string(),
                buyer_address: owner.to_string(),
                amount: delta,
                slot,
            });
        }
    }

    None
}

fn collect_instructions(value: &Value) -> Vec<&Value> {
    let mut instructions = Vec::new();

    if let Some(top_level) = value
        .pointer("/transaction/message/instructions")
        .and_then(Value::as_array)
    {
        instructions.extend(top_level.iter());
    }

    if let Some(inner_groups) = value
        .pointer("/meta/innerInstructions")
        .and_then(Value::as_array)
    {
        for group in inner_groups {
            if let Some(inner) = group.get("instructions").and_then(Value::as_array) {
                instructions.extend(inner.iter());
            }
        }
    }

    instructions
}

fn token_amount(info: &Value) -> Option<f64> {
    info.pointer("/tokenAmount/uiAmount")
        .and_then(Value::as_f64)
        .or_else(|| {
            info.pointer("/tokenAmount/uiAmountString")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<f64>().ok())
        })
        .or_else(|| {
            info.get("amount")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<f64>().ok())
        })
        .or_else(|| info.get("amount").and_then(Value::as_f64))
}

fn ui_amount(balance: &Value) -> Option<f64> {
    balance
        .pointer("/uiTokenAmount/uiAmount")
        .and_then(Value::as_f64)
        .or_else(|| {
            balance
                .pointer("/uiTokenAmount/uiAmountString")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<f64>().ok())
        })
}

fn looks_like_pubkey(value: &str) -> bool {
    (32..=44).contains(&value.len()) && value.chars().all(|c| c.is_ascii_alphanumeric())
}

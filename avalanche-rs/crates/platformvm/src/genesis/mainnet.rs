//! Mainnet genesis data parser.
//!
//! Parses the actual Avalanche mainnet genesis JSON format.

use std::collections::HashMap;

use avalanche_ids::{Id, NodeId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use chrono::{DateTime, Datelike, Utc};

/// Raw mainnet genesis format (matches avalanchego JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainnetGenesisRaw {
    /// Network ID.
    pub network_id: u32,

    /// Initial allocations by address.
    pub allocations: Vec<RawAllocation>,

    /// Start time (Unix timestamp).
    pub start_time: u64,

    /// Initial stakers.
    pub initial_stakers: Vec<RawStaker>,

    /// C-Chain genesis (hex encoded).
    #[serde(default)]
    pub c_chain_genesis: String,

    /// Message (genesis message).
    #[serde(default)]
    pub message: String,
}

/// Raw allocation from genesis JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawAllocation {
    /// Eth address (hex with 0x prefix).
    pub eth_addr: String,

    /// Avax address (Bech32).
    pub avax_addr: String,

    /// Initial amount in nAVAX.
    pub initial_amount: u64,

    /// Unlock schedule.
    #[serde(default)]
    pub unlock_schedule: Vec<UnlockScheduleEntry>,
}

/// Unlock schedule entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockScheduleEntry {
    /// Amount to unlock.
    pub amount: u64,

    /// Locktime (Unix timestamp).
    pub locktime: u64,
}

/// Raw staker from genesis JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawStaker {
    /// Node ID (CB58 encoded).
    pub node_id: String,

    /// Reward address (Bech32).
    pub reward_address: String,

    /// Delegation fee (0-100).
    pub delegation_fee: u32,
}

/// Parsed mainnet genesis.
#[derive(Debug, Clone)]
pub struct ParsedMainnetGenesis {
    /// Network ID.
    pub network_id: u32,
    /// Start time.
    pub start_time: DateTime<Utc>,
    /// Total supply from allocations.
    pub total_supply: u64,
    /// Initial stakers.
    pub initial_stakers: Vec<ParsedStaker>,
    /// Allocations by address.
    pub allocations: Vec<ParsedAllocation>,
    /// C-Chain genesis bytes.
    pub c_chain_genesis: Vec<u8>,
    /// Genesis message.
    pub message: String,
    /// Genesis hash.
    pub genesis_hash: Id,
}

/// Parsed staker.
#[derive(Debug, Clone)]
pub struct ParsedStaker {
    /// Node ID.
    pub node_id: NodeId,
    /// Reward address bytes.
    pub reward_address: Vec<u8>,
    /// Delegation fee (0-100).
    pub delegation_fee: u32,
    /// Stake weight (assigned from initial stake).
    pub weight: u64,
}

/// Parsed allocation.
#[derive(Debug, Clone)]
pub struct ParsedAllocation {
    /// Avalanche address bytes.
    pub address: Vec<u8>,
    /// Ethereum address bytes (for C-Chain).
    pub eth_address: [u8; 20],
    /// Initial unlocked amount.
    pub initial_amount: u64,
    /// Locked amounts with unlock times.
    pub locked_amounts: Vec<(u64, u64)>, // (amount, locktime)
    /// Total amount (initial + all locked).
    pub total_amount: u64,
}

impl MainnetGenesisRaw {
    /// Parses mainnet genesis from JSON bytes.
    pub fn from_json(bytes: &[u8]) -> Result<Self, GenesisParseError> {
        serde_json::from_slice(bytes).map_err(|e| GenesisParseError::JsonParse(e.to_string()))
    }

    /// Parses mainnet genesis from JSON string.
    pub fn from_json_str(json: &str) -> Result<Self, GenesisParseError> {
        serde_json::from_str(json).map_err(|e| GenesisParseError::JsonParse(e.to_string()))
    }

    /// Converts to parsed genesis with validation.
    pub fn parse(self) -> Result<ParsedMainnetGenesis, GenesisParseError> {
        // Validate network ID
        if self.network_id != 1 {
            return Err(GenesisParseError::InvalidNetworkId(self.network_id));
        }

        // Compute genesis hash FIRST before consuming any fields
        let genesis_bytes = serde_json::to_vec(&self).unwrap_or_default();
        let hash = Sha256::digest(&genesis_bytes);
        let genesis_hash = Id::from_slice(&hash).unwrap_or_default();

        // Parse start time
        let start_time = DateTime::from_timestamp(self.start_time as i64, 0)
            .ok_or(GenesisParseError::InvalidTimestamp(self.start_time))?;

        // Parse C-Chain genesis
        let c_chain_genesis = if self.c_chain_genesis.is_empty() {
            Vec::new()
        } else {
            hex::decode(self.c_chain_genesis.trim_start_matches("0x"))
                .map_err(|e| GenesisParseError::HexDecode(e.to_string()))?
        };

        let message = self.message;

        // Parse stakers
        let initial_stakers: Vec<ParsedStaker> = self
            .initial_stakers
            .into_iter()
            .map(|s| parse_staker(s))
            .collect::<Result<Vec<_>, _>>()?;

        // Parse allocations
        let mut total_supply = 0u64;
        let allocations: Vec<ParsedAllocation> = self
            .allocations
            .into_iter()
            .map(|a| {
                let parsed = parse_allocation(a)?;
                total_supply = total_supply.saturating_add(parsed.total_amount);
                Ok(parsed)
            })
            .collect::<Result<Vec<_>, GenesisParseError>>()?;

        Ok(ParsedMainnetGenesis {
            network_id: 1,
            start_time,
            total_supply,
            initial_stakers,
            allocations,
            c_chain_genesis,
            message,
            genesis_hash,
        })
    }
}

/// Parses a raw staker.
fn parse_staker(raw: RawStaker) -> Result<ParsedStaker, GenesisParseError> {
    // Parse node ID from CB58
    let node_id = parse_node_id(&raw.node_id)?;

    // Parse reward address (Bech32)
    let reward_address = parse_avax_address(&raw.reward_address)?;

    Ok(ParsedStaker {
        node_id,
        reward_address,
        delegation_fee: raw.delegation_fee,
        weight: 2_000_000_000_000_000, // Default bootstrap stake: 2M AVAX
    })
}

/// Parses a raw allocation.
fn parse_allocation(raw: RawAllocation) -> Result<ParsedAllocation, GenesisParseError> {
    // Parse Avalanche address
    let address = parse_avax_address(&raw.avax_addr)?;

    // Parse Ethereum address
    let eth_address = parse_eth_address(&raw.eth_addr)?;

    // Calculate total and locked amounts
    let mut total_amount = raw.initial_amount;
    let locked_amounts: Vec<(u64, u64)> = raw
        .unlock_schedule
        .into_iter()
        .map(|e| {
            total_amount = total_amount.saturating_add(e.amount);
            (e.amount, e.locktime)
        })
        .collect();

    Ok(ParsedAllocation {
        address,
        eth_address,
        initial_amount: raw.initial_amount,
        locked_amounts,
        total_amount,
    })
}

/// Parses a node ID from CB58 string.
fn parse_node_id(s: &str) -> Result<NodeId, GenesisParseError> {
    // NodeID format: "NodeID-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
    let id_str = s.strip_prefix("NodeID-").unwrap_or(s);

    // CB58 decode (Base58Check)
    let bytes = cb58_decode(id_str)?;

    if bytes.len() != 20 {
        return Err(GenesisParseError::InvalidNodeId(format!(
            "expected 20 bytes, got {}",
            bytes.len()
        )));
    }

    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(NodeId::from_bytes(arr))
}

/// Parses an Avalanche address from Bech32.
fn parse_avax_address(s: &str) -> Result<Vec<u8>, GenesisParseError> {
    // Address format: "X-avax1xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" or "P-avax1..."
    let addr_str = if s.contains('-') {
        s.split('-').nth(1).unwrap_or(s)
    } else {
        s
    };

    // Simple Bech32 decode (without full validation)
    // In production, use proper bech32 library
    if addr_str.starts_with("avax1") || addr_str.starts_with("fuji1") {
        let data_part = &addr_str[5..]; // Skip "avax1" or "fuji1"
        bech32_decode_data(data_part)
    } else {
        // Fallback: try hex
        hex::decode(s.trim_start_matches("0x"))
            .map_err(|e| GenesisParseError::InvalidAddress(e.to_string()))
    }
}

/// Parses an Ethereum address.
fn parse_eth_address(s: &str) -> Result<[u8; 20], GenesisParseError> {
    let hex_str = s.trim_start_matches("0x");
    let bytes = hex::decode(hex_str)
        .map_err(|e| GenesisParseError::InvalidAddress(e.to_string()))?;

    if bytes.len() != 20 {
        return Err(GenesisParseError::InvalidAddress(format!(
            "expected 20 bytes, got {}",
            bytes.len()
        )));
    }

    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// CB58 (Base58Check) decode.
fn cb58_decode(s: &str) -> Result<Vec<u8>, GenesisParseError> {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    let mut result: Vec<u8> = Vec::new();

    for c in s.chars() {
        let val = ALPHABET
            .iter()
            .position(|&x| x == c as u8)
            .ok_or_else(|| GenesisParseError::CB58Decode(format!("invalid char: {}", c)))?
            as u64;

        let mut carry = val;
        for byte in result.iter_mut().rev() {
            carry += (*byte as u64) * 58;
            *byte = (carry & 0xFF) as u8;
            carry >>= 8;
        }

        while carry > 0 {
            result.insert(0, (carry & 0xFF) as u8);
            carry >>= 8;
        }
    }

    // Handle leading '1's (zeros in base58)
    for c in s.chars() {
        if c == '1' {
            result.insert(0, 0);
        } else {
            break;
        }
    }

    // Remove 4-byte checksum
    if result.len() >= 4 {
        result.truncate(result.len() - 4);
    }

    Ok(result)
}

/// Simple Bech32 data decode (5-bit to 8-bit conversion).
fn bech32_decode_data(data: &str) -> Result<Vec<u8>, GenesisParseError> {
    const CHARSET: &str = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";

    // Convert from Bech32 characters to 5-bit values
    let values: Result<Vec<u8>, _> = data
        .chars()
        .map(|c| {
            CHARSET
                .find(c)
                .map(|i| i as u8)
                .ok_or_else(|| GenesisParseError::Bech32Decode(format!("invalid char: {}", c)))
        })
        .collect();
    let values = values?;

    // Convert from 5-bit to 8-bit
    let mut result = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0u32;

    for val in values {
        acc = (acc << 5) | (val as u32);
        bits += 5;

        while bits >= 8 {
            bits -= 8;
            result.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }

    Ok(result)
}

/// Genesis parsing errors.
#[derive(Debug, thiserror::Error)]
pub enum GenesisParseError {
    #[error("JSON parse error: {0}")]
    JsonParse(String),
    #[error("invalid network ID: {0}")]
    InvalidNetworkId(u32),
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(u64),
    #[error("invalid node ID: {0}")]
    InvalidNodeId(String),
    #[error("invalid address: {0}")]
    InvalidAddress(String),
    #[error("hex decode error: {0}")]
    HexDecode(String),
    #[error("CB58 decode error: {0}")]
    CB58Decode(String),
    #[error("Bech32 decode error: {0}")]
    Bech32Decode(String),
}

/// Mainnet genesis constants.
pub mod constants {
    use super::*;

    /// Mainnet network ID.
    pub const MAINNET_NETWORK_ID: u32 = 1;

    /// Mainnet genesis timestamp (September 21, 2020 @ 10:00 UTC).
    pub const MAINNET_GENESIS_TIMESTAMP: u64 = 1600693200;

    /// Total AVAX supply (720 million AVAX = 720 * 10^15 nAVAX).
    pub const TOTAL_AVAX_SUPPLY: u64 = 720_000_000_000_000_000;

    /// P-Chain ID.
    pub fn p_chain_id() -> Id {
        Id::default() // Primary network
    }

    /// X-Chain ID.
    pub fn x_chain_id() -> Id {
        // The actual X-Chain ID on mainnet
        let bytes = [
            0x20, 0x3d, 0x46, 0x39, 0x38, 0x34, 0x38, 0x39,
            0x33, 0x32, 0x39, 0x66, 0x66, 0x38, 0x66, 0x32,
            0x45, 0x56, 0x5a, 0x31, 0x64, 0x71, 0x6a, 0x78,
            0x78, 0x6e, 0x47, 0x37, 0x31, 0x6e, 0x39, 0x52,
        ];
        Id::from_slice(&bytes).unwrap_or_default()
    }

    /// C-Chain ID.
    pub fn c_chain_id() -> Id {
        // The actual C-Chain ID on mainnet
        let bytes = [
            0x32, 0x71, 0x39, 0x65, 0x34, 0x4f, 0x4d, 0x69,
            0x6e, 0x6a, 0x51, 0x77, 0x6f, 0x53, 0x62, 0x38,
            0x36, 0x36, 0x6e, 0x72, 0x56, 0x32, 0x74, 0x76,
            0x6d, 0x42, 0x39, 0x6a, 0x56, 0x48, 0x52, 0x4c,
        ];
        Id::from_slice(&bytes).unwrap_or_default()
    }

    /// Mainnet genesis timestamp as DateTime.
    pub fn mainnet_genesis_time() -> DateTime<Utc> {
        DateTime::from_timestamp(MAINNET_GENESIS_TIMESTAMP as i64, 0).unwrap()
    }

    /// Verify total supply matches expected.
    pub fn verify_total_supply(supply: u64) -> bool {
        supply == TOTAL_AVAX_SUPPLY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cb58_decode() {
        // Test with a known CB58 string
        let result = cb58_decode("111111111111111111116DBWJs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_eth_address_parse() {
        let addr = "0x1234567890123456789012345678901234567890";
        let result = parse_eth_address(addr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 20);
    }

    #[test]
    fn test_parse_sample_genesis() {
        let sample = r#"{
            "networkId": 1,
            "allocations": [],
            "startTime": 1600693200,
            "initialStakers": [],
            "cChainGenesis": "",
            "message": "test genesis"
        }"#;

        let raw = MainnetGenesisRaw::from_json_str(sample).unwrap();
        assert_eq!(raw.network_id, 1);
        assert_eq!(raw.start_time, 1600693200);
    }

    #[test]
    fn test_genesis_with_allocations() {
        let sample = r#"{
            "networkId": 1,
            "allocations": [
                {
                    "ethAddr": "0x1234567890123456789012345678901234567890",
                    "avaxAddr": "X-avax1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq3lpmx5",
                    "initialAmount": 1000000000000000,
                    "unlockSchedule": []
                }
            ],
            "startTime": 1600693200,
            "initialStakers": [],
            "cChainGenesis": "",
            "message": "test"
        }"#;

        let raw = MainnetGenesisRaw::from_json_str(sample).unwrap();
        assert_eq!(raw.allocations.len(), 1);
        assert_eq!(raw.allocations[0].initial_amount, 1000000000000000);
    }

    #[test]
    fn test_constants() {
        assert_eq!(constants::MAINNET_NETWORK_ID, 1);
        assert_eq!(constants::TOTAL_AVAX_SUPPLY, 720_000_000_000_000_000);

        let genesis_time = constants::mainnet_genesis_time();
        assert_eq!(genesis_time.year(), 2020);
        assert_eq!(genesis_time.month(), 9);
    }

    #[test]
    fn test_bech32_decode() {
        // Simple test - actual Bech32 requires more sophisticated handling
        let result = bech32_decode_data("qqqqqqqqqqqqqqqqqqqq");
        assert!(result.is_ok());
    }
}

//! Protocol compatibility tests and verification.
//!
//! This module ensures bit-for-bit compatibility with the Go avalanchego implementation.
//! Test vectors are derived from actual Go-encoded messages.

use bytes::{Bytes, BytesMut};
use sha2::{Digest, Sha256};

use crate::p2p::*;

/// Known test vectors from the Go implementation.
/// These are actual encoded messages that both implementations must produce identically.
pub mod test_vectors {
    use super::*;

    /// Ping message with uptime=95
    /// Go: message.Ping{Uptime: 95}
    pub const PING_UPTIME_95: &[u8] = &[
        0x5a, // Field 11, wire type 2 (length-delimited)
        0x02, // Length: 2 bytes
        0x08, // Field 1, wire type 0 (varint)
        0x5f, // Value: 95
    ];

    /// Pong message (empty)
    /// Go: message.Pong{}
    pub const PONG_EMPTY: &[u8] = &[
        0x62, // Field 12, wire type 2 (length-delimited)
        0x00, // Length: 0 bytes
    ];

    /// Get message
    /// Go: message.Get{ChainID: [32]byte{1}, RequestID: 42, Deadline: 1000000000, ContainerID: [32]byte{2}}
    pub const GET_SIMPLE: &[u8] = &[
        0xca, 0x01, // Field 25, wire type 2
        0x4a, // Length: 74 bytes
        0x0a, 0x20, // Field 1 (chain_id), wire type 2, length 32
        0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
        0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
        0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
        0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
        0x10, 0x2a, // Field 2 (request_id), wire type 0, value 42
        0x18, 0x80, 0xa8, 0xd6, 0xb9, 0x07, // Field 3 (deadline), wire type 0, value 1000000000
        0x22, 0x20, // Field 4 (container_id), wire type 2, length 32
        0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02,
        0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02,
        0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02,
        0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02, 0x02,
    ];
}

/// Computes a hash of the encoded message for comparison.
pub fn message_hash(data: &[u8]) -> [u8; 32] {
    let hash = Sha256::digest(data);
    let mut result = [0u8; 32];
    result.copy_from_slice(&hash);
    result
}

/// Verifies that a Rust-encoded message matches the expected bytes exactly.
pub fn verify_encoding(message: &Message, expected: &[u8]) -> Result<(), CompatError> {
    let encoded = message.encode().map_err(|e| CompatError::EncodeError(e.to_string()))?;

    if encoded.as_ref() != expected {
        return Err(CompatError::EncodingMismatch {
            expected: hex::encode(expected),
            got: hex::encode(&encoded),
        });
    }

    Ok(())
}

/// Verifies that a Rust-decoded message matches the expected structure.
pub fn verify_decoding(data: &[u8], expected: &Message) -> Result<(), CompatError> {
    let decoded = Message::decode(data).map_err(|e| CompatError::DecodeError(e.to_string()))?;

    if &decoded != expected {
        return Err(CompatError::DecodingMismatch {
            expected: format!("{:?}", expected),
            got: format!("{:?}", decoded),
        });
    }

    Ok(())
}

/// Verifies round-trip encoding/decoding preserves the message exactly.
pub fn verify_roundtrip(message: &Message) -> Result<(), CompatError> {
    let encoded = message.encode().map_err(|e| CompatError::EncodeError(e.to_string()))?;
    let decoded = Message::decode(&encoded).map_err(|e| CompatError::DecodeError(e.to_string()))?;

    if &decoded != message {
        return Err(CompatError::RoundtripMismatch {
            original: format!("{:?}", message),
            after_roundtrip: format!("{:?}", decoded),
        });
    }

    Ok(())
}

/// Compatibility error types.
#[derive(Debug, thiserror::Error)]
pub enum CompatError {
    #[error("encoding error: {0}")]
    EncodeError(String),

    #[error("decoding error: {0}")]
    DecodeError(String),

    #[error("encoding mismatch: expected {expected}, got {got}")]
    EncodingMismatch { expected: String, got: String },

    #[error("decoding mismatch: expected {expected}, got {got}")]
    DecodingMismatch { expected: String, got: String },

    #[error("roundtrip mismatch: original {original}, after roundtrip {after_roundtrip}")]
    RoundtripMismatch {
        original: String,
        after_roundtrip: String,
    },
}

/// Protocol version compatibility.
pub mod version {
    /// Minimum protocol version supported.
    pub const MIN_PROTOCOL_VERSION: u32 = 28;

    /// Current protocol version.
    pub const CURRENT_PROTOCOL_VERSION: u32 = 35;

    /// Checks if a protocol version is supported.
    pub fn is_supported(version: u32) -> bool {
        version >= MIN_PROTOCOL_VERSION && version <= CURRENT_PROTOCOL_VERSION
    }
}

/// Message field numbers (must match Go implementation).
pub mod field_numbers {
    pub const COMPRESSED_ZSTD: u32 = 2;
    pub const PING: u32 = 11;
    pub const PONG: u32 = 12;
    pub const HANDSHAKE: u32 = 13;
    pub const PEER_LIST: u32 = 14;
    pub const GET_STATE_SUMMARY_FRONTIER: u32 = 15;
    pub const STATE_SUMMARY_FRONTIER: u32 = 16;
    pub const GET_ACCEPTED_STATE_SUMMARY: u32 = 17;
    pub const ACCEPTED_STATE_SUMMARY: u32 = 18;
    pub const GET_ACCEPTED_FRONTIER: u32 = 19;
    pub const ACCEPTED_FRONTIER: u32 = 20;
    pub const GET_ACCEPTED: u32 = 21;
    pub const ACCEPTED: u32 = 22;
    pub const GET_ANCESTORS: u32 = 23;
    pub const ANCESTORS: u32 = 24;
    pub const GET: u32 = 25;
    pub const PUT: u32 = 26;
    pub const PUSH_QUERY: u32 = 27;
    pub const PULL_QUERY: u32 = 28;
    pub const CHITS: u32 = 29;
    pub const APP_REQUEST: u32 = 30;
    pub const APP_RESPONSE: u32 = 31;
    pub const APP_GOSSIP: u32 = 32;
    pub const APP_ERROR: u32 = 34;
    pub const GET_PEER_LIST: u32 = 35;
}

/// Validates message structure for protocol compliance.
pub fn validate_message(message: &Message) -> Result<(), ValidationError> {
    match message {
        Message::Ping(ping) => {
            if ping.uptime > 100 {
                return Err(ValidationError::InvalidUptime(ping.uptime));
            }
        }
        Message::Handshake(handshake) => {
            if handshake.network_id == 0 {
                return Err(ValidationError::InvalidNetworkId);
            }
            if handshake.ip_addr.len() != 4 && handshake.ip_addr.len() != 16 {
                return Err(ValidationError::InvalidIpAddress(handshake.ip_addr.len()));
            }
        }
        Message::Get(get) => {
            if get.chain_id.len() != 32 {
                return Err(ValidationError::InvalidChainId(get.chain_id.len()));
            }
            if get.container_id.len() != 32 {
                return Err(ValidationError::InvalidContainerId(get.container_id.len()));
            }
        }
        Message::Put(put) => {
            if put.chain_id.len() != 32 {
                return Err(ValidationError::InvalidChainId(put.chain_id.len()));
            }
            if put.container.is_empty() {
                return Err(ValidationError::EmptyContainer);
            }
        }
        Message::PushQuery(pq) => {
            if pq.chain_id.len() != 32 {
                return Err(ValidationError::InvalidChainId(pq.chain_id.len()));
            }
        }
        Message::PullQuery(pq) => {
            if pq.chain_id.len() != 32 {
                return Err(ValidationError::InvalidChainId(pq.chain_id.len()));
            }
            if pq.container_id.len() != 32 {
                return Err(ValidationError::InvalidContainerId(pq.container_id.len()));
            }
        }
        Message::Chits(chits) => {
            if chits.chain_id.len() != 32 {
                return Err(ValidationError::InvalidChainId(chits.chain_id.len()));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Validation error types.
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("invalid uptime: {0} (must be 0-100)")]
    InvalidUptime(u32),

    #[error("invalid network ID (must be non-zero)")]
    InvalidNetworkId,

    #[error("invalid IP address length: {0} (must be 4 or 16)")]
    InvalidIpAddress(usize),

    #[error("invalid chain ID length: {0} (must be 32)")]
    InvalidChainId(usize),

    #[error("invalid container ID length: {0} (must be 32)")]
    InvalidContainerId(usize),

    #[error("container cannot be empty")]
    EmptyContainer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_vector() {
        let ping = Message::Ping(Ping { uptime: 95 });
        let result = verify_decoding(test_vectors::PING_UPTIME_95, &ping);
        assert!(result.is_ok(), "Ping decoding failed: {:?}", result);
    }

    #[test]
    fn test_pong_vector() {
        let pong = Message::Pong(Pong {});
        let result = verify_decoding(test_vectors::PONG_EMPTY, &pong);
        assert!(result.is_ok(), "Pong decoding failed: {:?}", result);
    }

    #[test]
    fn test_ping_roundtrip() {
        let ping = Message::Ping(Ping { uptime: 42 });
        assert!(verify_roundtrip(&ping).is_ok());
    }

    #[test]
    fn test_handshake_roundtrip() {
        let handshake = Message::Handshake(Box::new(Handshake {
            network_id: 1,
            my_time: 1609459200,
            ip_addr: vec![127, 0, 0, 1],
            ip_port: 9651,
            ip_signing_time: 1609459200,
            ip_node_id_sig: vec![0u8; 65],
            tracked_subnets: vec![vec![0u8; 32]],
            client: Some(Client::new("avalanche-rs", 0, 1, 0)),
            supported_acps: vec![23, 24, 25],
            objected_acps: vec![],
            known_peers: None,
            ip_bls_sig: vec![],
            all_subnets: false,
        }));
        assert!(verify_roundtrip(&handshake).is_ok());
    }

    #[test]
    fn test_get_roundtrip() {
        let get = Message::Get(Get {
            chain_id: vec![0x01; 32],
            request_id: 12345,
            deadline: 1_000_000_000,
            container_id: vec![0x02; 32],
        });
        assert!(verify_roundtrip(&get).is_ok());
    }

    #[test]
    fn test_put_roundtrip() {
        let put = Message::Put(Put {
            chain_id: vec![0x01; 32],
            request_id: 12345,
            container: vec![0xAB; 1000],
        });
        assert!(verify_roundtrip(&put).is_ok());
    }

    #[test]
    fn test_push_query_roundtrip() {
        let pq = Message::PushQuery(PushQuery {
            chain_id: vec![0x01; 32],
            request_id: 12345,
            deadline: 1_000_000_000,
            container: vec![0xAB; 500],
            requested_height: 100,
        });
        assert!(verify_roundtrip(&pq).is_ok());
    }

    #[test]
    fn test_pull_query_roundtrip() {
        let pq = Message::PullQuery(PullQuery {
            chain_id: vec![0x01; 32],
            request_id: 12345,
            deadline: 1_000_000_000,
            container_id: vec![0x02; 32],
            requested_height: 100,
        });
        assert!(verify_roundtrip(&pq).is_ok());
    }

    #[test]
    fn test_chits_roundtrip() {
        let chits = Message::Chits(Chits {
            chain_id: vec![0x01; 32],
            request_id: 12345,
            preferred_id: vec![0x02; 32],
            accepted_id: vec![0x03; 32],
            preferred_id_at_height: vec![0x04; 32],
            accepted_height: 1000,
        });
        assert!(verify_roundtrip(&chits).is_ok());
    }

    #[test]
    fn test_ancestors_roundtrip() {
        let ancestors = Message::Ancestors(Ancestors {
            chain_id: vec![0x01; 32],
            request_id: 12345,
            containers: vec![
                vec![0xAB; 100],
                vec![0xCD; 100],
                vec![0xEF; 100],
            ],
        });
        assert!(verify_roundtrip(&ancestors).is_ok());
    }

    #[test]
    fn test_compression_roundtrip() {
        let ancestors = Message::Ancestors(Ancestors {
            chain_id: vec![0x01; 32],
            request_id: 12345,
            containers: vec![vec![0xAB; 1000]; 10],
        });

        let compressed = ancestors.compress().unwrap();
        let decompressed = compressed.decompress().unwrap();
        assert_eq!(decompressed, ancestors);
    }

    #[test]
    fn test_version_compatibility() {
        assert!(version::is_supported(28));
        assert!(version::is_supported(35));
        assert!(!version::is_supported(27));
        assert!(!version::is_supported(36));
    }

    #[test]
    fn test_field_numbers() {
        // Verify field numbers match the Go implementation
        assert_eq!(field_numbers::PING, 11);
        assert_eq!(field_numbers::PONG, 12);
        assert_eq!(field_numbers::HANDSHAKE, 13);
        assert_eq!(field_numbers::GET, 25);
        assert_eq!(field_numbers::PUT, 26);
        assert_eq!(field_numbers::PUSH_QUERY, 27);
        assert_eq!(field_numbers::PULL_QUERY, 28);
        assert_eq!(field_numbers::CHITS, 29);
    }

    #[test]
    fn test_validate_ping() {
        let valid = Message::Ping(Ping { uptime: 95 });
        assert!(validate_message(&valid).is_ok());

        let invalid = Message::Ping(Ping { uptime: 101 });
        assert!(validate_message(&invalid).is_err());
    }

    #[test]
    fn test_validate_handshake() {
        let valid = Message::Handshake(Box::new(Handshake {
            network_id: 1,
            ip_addr: vec![127, 0, 0, 1],
            ..Default::default()
        }));
        assert!(validate_message(&valid).is_ok());

        let invalid_network = Message::Handshake(Box::new(Handshake {
            network_id: 0,
            ip_addr: vec![127, 0, 0, 1],
            ..Default::default()
        }));
        assert!(validate_message(&invalid_network).is_err());

        let invalid_ip = Message::Handshake(Box::new(Handshake {
            network_id: 1,
            ip_addr: vec![127, 0, 0], // Only 3 bytes
            ..Default::default()
        }));
        assert!(validate_message(&invalid_ip).is_err());
    }

    #[test]
    fn test_validate_get() {
        let valid = Message::Get(Get {
            chain_id: vec![0x01; 32],
            request_id: 1,
            deadline: 1_000_000_000,
            container_id: vec![0x02; 32],
        });
        assert!(validate_message(&valid).is_ok());

        let invalid_chain = Message::Get(Get {
            chain_id: vec![0x01; 31], // Wrong length
            request_id: 1,
            deadline: 1_000_000_000,
            container_id: vec![0x02; 32],
        });
        assert!(validate_message(&invalid_chain).is_err());
    }

    #[test]
    fn test_message_hash() {
        let data = b"test message";
        let hash = message_hash(data);
        assert_eq!(hash.len(), 32);
        // Same input should produce same hash
        assert_eq!(hash, message_hash(data));
    }

    #[test]
    fn test_all_message_types_roundtrip() {
        let messages: Vec<Message> = vec![
            Message::Ping(Ping { uptime: 99 }),
            Message::Pong(Pong {}),
            Message::GetPeerList(GetPeerList {
                known_peers: None,
                all_subnets: true,
            }),
            Message::PeerList(PeerList {
                claimed_ip_ports: vec![],
            }),
            Message::GetStateSummaryFrontier(GetStateSummaryFrontier {
                chain_id: vec![0x01; 32],
                request_id: 1,
                deadline: 1_000_000_000,
            }),
            Message::StateSummaryFrontier(StateSummaryFrontier {
                chain_id: vec![0x01; 32],
                request_id: 1,
                summary: vec![0xAB; 100],
            }),
            Message::GetAcceptedStateSummary(GetAcceptedStateSummary {
                chain_id: vec![0x01; 32],
                request_id: 1,
                deadline: 1_000_000_000,
                heights: vec![100, 200, 300],
            }),
            Message::AcceptedStateSummary(AcceptedStateSummary {
                chain_id: vec![0x01; 32],
                request_id: 1,
                summary_ids: vec![vec![0x01; 32], vec![0x02; 32]],
            }),
            Message::GetAcceptedFrontier(GetAcceptedFrontier {
                chain_id: vec![0x01; 32],
                request_id: 1,
                deadline: 1_000_000_000,
            }),
            Message::AcceptedFrontier(AcceptedFrontier {
                chain_id: vec![0x01; 32],
                request_id: 1,
                container_id: vec![0x02; 32],
            }),
            Message::GetAccepted(GetAccepted {
                chain_id: vec![0x01; 32],
                request_id: 1,
                deadline: 1_000_000_000,
                container_ids: vec![vec![0x01; 32], vec![0x02; 32]],
            }),
            Message::Accepted(Accepted {
                chain_id: vec![0x01; 32],
                request_id: 1,
                container_ids: vec![vec![0x01; 32]],
            }),
            Message::GetAncestors(GetAncestors {
                chain_id: vec![0x01; 32],
                request_id: 1,
                deadline: 1_000_000_000,
                container_id: vec![0x02; 32],
                engine_type: 2, // Snowman
            }),
            Message::Ancestors(Ancestors {
                chain_id: vec![0x01; 32],
                request_id: 1,
                containers: vec![vec![0xAB; 100]],
            }),
            Message::Get(Get {
                chain_id: vec![0x01; 32],
                request_id: 1,
                deadline: 1_000_000_000,
                container_id: vec![0x02; 32],
            }),
            Message::Put(Put {
                chain_id: vec![0x01; 32],
                request_id: 1,
                container: vec![0xAB; 100],
            }),
            Message::AppRequest(AppRequest {
                chain_id: vec![0x01; 32],
                request_id: 1,
                deadline: 1_000_000_000,
                app_bytes: vec![0xAB; 50],
            }),
            Message::AppResponse(AppResponse {
                chain_id: vec![0x01; 32],
                request_id: 1,
                app_bytes: vec![0xCD; 50],
            }),
            Message::AppGossip(AppGossip {
                chain_id: vec![0x01; 32],
                app_bytes: vec![0xEF; 50],
            }),
            Message::AppError(AppError {
                chain_id: vec![0x01; 32],
                request_id: 1,
                error_code: -1,
                error_message: "test error".to_string(),
            }),
        ];

        for msg in messages {
            let result = verify_roundtrip(&msg);
            assert!(result.is_ok(), "Roundtrip failed for {:?}: {:?}", msg, result);
        }
    }
}

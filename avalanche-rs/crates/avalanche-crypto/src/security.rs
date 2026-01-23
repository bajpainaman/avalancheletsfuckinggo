//! Security utilities and hardening.
//!
//! This module provides security-critical utilities including:
//! - Constant-time comparison to prevent timing attacks
//! - Secure random number generation
//! - Input validation and sanitization
//! - Secure memory handling

use std::sync::atomic::{AtomicBool, Ordering};
use zeroize::Zeroize;

/// Constant-time comparison to prevent timing attacks.
///
/// This function compares two byte slices in constant time, meaning the
/// execution time does not depend on where the slices differ.
///
/// # Security
///
/// This MUST be used when comparing:
/// - Cryptographic signatures
/// - Message authentication codes (MACs)
/// - Password hashes
/// - Any security-sensitive data
///
/// # Example
///
/// ```
/// use avalanche_crypto::security::constant_time_eq;
///
/// let a = [1, 2, 3, 4];
/// let b = [1, 2, 3, 4];
/// assert!(constant_time_eq(&a, &b));
/// ```
#[inline(never)]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }

    result == 0
}

/// Secure comparison for IDs with constant-time equality.
#[inline(never)]
pub fn constant_time_eq_32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut result = 0u8;
    for i in 0..32 {
        result |= a[i] ^ b[i];
    }
    result == 0
}

/// Securely zeroes memory.
///
/// This function ensures that sensitive data is overwritten before being
/// deallocated, preventing recovery through memory analysis.
pub fn secure_zero(data: &mut [u8]) {
    data.zeroize();
}

/// Wrapper for sensitive data that zeroes on drop.
#[derive(Clone)]
pub struct SecureBytes {
    data: Vec<u8>,
}

impl SecureBytes {
    /// Creates a new SecureBytes from a vector.
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Creates a new SecureBytes with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    /// Returns a reference to the inner data.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Returns a mutable reference to the inner data.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Returns the length of the data.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Extends with bytes.
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.data.extend_from_slice(other);
    }
}

impl Drop for SecureBytes {
    fn drop(&mut self) {
        self.data.zeroize();
    }
}

impl From<Vec<u8>> for SecureBytes {
    fn from(data: Vec<u8>) -> Self {
        Self::new(data)
    }
}

impl AsRef<[u8]> for SecureBytes {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

/// Input validation utilities.
pub mod validation {
    use super::*;

    /// Maximum allowed message size (16 MB).
    pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

    /// Maximum allowed block size (2 MB).
    pub const MAX_BLOCK_SIZE: usize = 2 * 1024 * 1024;

    /// Maximum allowed transaction size (128 KB).
    pub const MAX_TX_SIZE: usize = 128 * 1024;

    /// Maximum number of transactions per block.
    pub const MAX_TXS_PER_BLOCK: usize = 10_000;

    /// Maximum call data size for EVM transactions.
    pub const MAX_CALLDATA_SIZE: usize = 128 * 1024;

    /// Maximum number of logs per transaction.
    pub const MAX_LOGS_PER_TX: usize = 10_000;

    /// Maximum recursion depth for JSON parsing.
    pub const MAX_JSON_DEPTH: usize = 64;

    /// Validates that a message size is within bounds.
    pub fn validate_message_size(size: usize) -> Result<(), ValidationError> {
        if size > MAX_MESSAGE_SIZE {
            return Err(ValidationError::MessageTooLarge {
                size,
                max: MAX_MESSAGE_SIZE,
            });
        }
        Ok(())
    }

    /// Validates that a block size is within bounds.
    pub fn validate_block_size(size: usize) -> Result<(), ValidationError> {
        if size > MAX_BLOCK_SIZE {
            return Err(ValidationError::BlockTooLarge {
                size,
                max: MAX_BLOCK_SIZE,
            });
        }
        Ok(())
    }

    /// Validates that a transaction size is within bounds.
    pub fn validate_tx_size(size: usize) -> Result<(), ValidationError> {
        if size > MAX_TX_SIZE {
            return Err(ValidationError::TransactionTooLarge {
                size,
                max: MAX_TX_SIZE,
            });
        }
        Ok(())
    }

    /// Validates a network ID.
    pub fn validate_network_id(id: u32) -> Result<(), ValidationError> {
        if id == 0 {
            return Err(ValidationError::InvalidNetworkId);
        }
        Ok(())
    }

    /// Validates a chain ID is 32 bytes.
    pub fn validate_chain_id(id: &[u8]) -> Result<(), ValidationError> {
        if id.len() != 32 {
            return Err(ValidationError::InvalidChainIdLength(id.len()));
        }
        Ok(())
    }

    /// Validates a node ID is 20 bytes.
    pub fn validate_node_id(id: &[u8]) -> Result<(), ValidationError> {
        if id.len() != 20 {
            return Err(ValidationError::InvalidNodeIdLength(id.len()));
        }
        Ok(())
    }

    /// Validates an Ethereum address is 20 bytes.
    pub fn validate_eth_address(addr: &[u8]) -> Result<(), ValidationError> {
        if addr.len() != 20 {
            return Err(ValidationError::InvalidAddressLength(addr.len()));
        }
        Ok(())
    }

    /// Validates a signature length.
    pub fn validate_signature_length(sig: &[u8], expected: usize) -> Result<(), ValidationError> {
        if sig.len() != expected {
            return Err(ValidationError::InvalidSignatureLength {
                got: sig.len(),
                expected,
            });
        }
        Ok(())
    }

    /// Validates a public key length.
    pub fn validate_pubkey_length(key: &[u8], expected: usize) -> Result<(), ValidationError> {
        if key.len() != expected {
            return Err(ValidationError::InvalidPublicKeyLength {
                got: key.len(),
                expected,
            });
        }
        Ok(())
    }

    /// Validates a timestamp is not too far in the future.
    pub fn validate_timestamp_not_future(
        timestamp: u64,
        max_drift_secs: u64,
    ) -> Result<(), ValidationError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if timestamp > now + max_drift_secs {
            return Err(ValidationError::TimestampTooFarInFuture {
                timestamp,
                max_allowed: now + max_drift_secs,
            });
        }
        Ok(())
    }

    /// Validates gas limit is within bounds.
    pub fn validate_gas_limit(gas: u64, max: u64) -> Result<(), ValidationError> {
        if gas > max {
            return Err(ValidationError::GasLimitExceeded { gas, max });
        }
        Ok(())
    }

    /// Validation error types.
    #[derive(Debug, thiserror::Error)]
    pub enum ValidationError {
        #[error("message too large: {size} bytes (max {max})")]
        MessageTooLarge { size: usize, max: usize },

        #[error("block too large: {size} bytes (max {max})")]
        BlockTooLarge { size: usize, max: usize },

        #[error("transaction too large: {size} bytes (max {max})")]
        TransactionTooLarge { size: usize, max: usize },

        #[error("invalid network ID (must be non-zero)")]
        InvalidNetworkId,

        #[error("invalid chain ID length: {0} (expected 32)")]
        InvalidChainIdLength(usize),

        #[error("invalid node ID length: {0} (expected 20)")]
        InvalidNodeIdLength(usize),

        #[error("invalid address length: {0} (expected 20)")]
        InvalidAddressLength(usize),

        #[error("invalid signature length: got {got}, expected {expected}")]
        InvalidSignatureLength { got: usize, expected: usize },

        #[error("invalid public key length: got {got}, expected {expected}")]
        InvalidPublicKeyLength { got: usize, expected: usize },

        #[error("timestamp too far in future: {timestamp} > {max_allowed}")]
        TimestampTooFarInFuture { timestamp: u64, max_allowed: u64 },

        #[error("gas limit exceeded: {gas} > {max}")]
        GasLimitExceeded { gas: u64, max: u64 },
    }
}

/// Rate limiting utilities.
pub mod rate_limit {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// Simple token bucket rate limiter.
    pub struct RateLimiter {
        buckets: Mutex<HashMap<String, TokenBucket>>,
        tokens_per_second: f64,
        burst_size: u64,
    }

    struct TokenBucket {
        tokens: f64,
        last_update: Instant,
    }

    impl RateLimiter {
        /// Creates a new rate limiter.
        pub fn new(tokens_per_second: f64, burst_size: u64) -> Self {
            Self {
                buckets: Mutex::new(HashMap::new()),
                tokens_per_second,
                burst_size,
            }
        }

        /// Checks if a request is allowed.
        pub fn check(&self, key: &str) -> bool {
            let mut buckets = self.buckets.lock().unwrap();
            let now = Instant::now();

            let bucket = buckets.entry(key.to_string()).or_insert(TokenBucket {
                tokens: self.burst_size as f64,
                last_update: now,
            });

            // Refill tokens based on time elapsed
            let elapsed = now.duration_since(bucket.last_update).as_secs_f64();
            bucket.tokens = (bucket.tokens + elapsed * self.tokens_per_second)
                .min(self.burst_size as f64);
            bucket.last_update = now;

            // Check if we have a token
            if bucket.tokens >= 1.0 {
                bucket.tokens -= 1.0;
                true
            } else {
                false
            }
        }

        /// Clears expired entries.
        pub fn cleanup(&self, max_age: Duration) {
            let mut buckets = self.buckets.lock().unwrap();
            let now = Instant::now();

            buckets.retain(|_, bucket| {
                now.duration_since(bucket.last_update) < max_age
            });
        }
    }

    /// Per-peer rate limiter.
    pub struct PeerRateLimiter {
        message_limiter: RateLimiter,
        byte_limiter: RateLimiter,
    }

    impl PeerRateLimiter {
        /// Creates a new peer rate limiter.
        pub fn new() -> Self {
            Self {
                // 100 messages per second, burst of 200
                message_limiter: RateLimiter::new(100.0, 200),
                // 10 MB per second, burst of 20 MB
                byte_limiter: RateLimiter::new(10_000_000.0, 20_000_000),
            }
        }

        /// Checks if a message from a peer is allowed.
        pub fn check_message(&self, peer_id: &str, message_size: usize) -> bool {
            if !self.message_limiter.check(peer_id) {
                return false;
            }

            // Check bytes (use multiple tokens for larger messages)
            let byte_key = format!("{}_bytes", peer_id);
            for _ in 0..message_size / 1000 + 1 {
                if !self.byte_limiter.check(&byte_key) {
                    return false;
                }
            }

            true
        }
    }

    impl Default for PeerRateLimiter {
        fn default() -> Self {
            Self::new()
        }
    }
}

/// Security audit checklist documentation.
pub mod audit {
    /// Security audit checklist for code review.
    ///
    /// # Critical Areas
    ///
    /// ## 1. Cryptographic Operations
    /// - [ ] All signature verifications use constant-time comparison
    /// - [ ] Private keys are zeroized after use
    /// - [ ] Random number generation uses secure RNG
    /// - [ ] No use of deprecated cryptographic algorithms
    ///
    /// ## 2. Input Validation
    /// - [ ] All external inputs are validated before use
    /// - [ ] Message sizes are bounded
    /// - [ ] Integer overflow/underflow is handled
    /// - [ ] No unbounded allocations from untrusted input
    ///
    /// ## 3. Memory Safety
    /// - [ ] No use of unsafe code (enforced by lints)
    /// - [ ] Sensitive data is zeroized on drop
    /// - [ ] No memory leaks in long-running operations
    ///
    /// ## 4. Consensus Safety
    /// - [ ] Byzantine fault tolerance maintained (f < n/3)
    /// - [ ] No equivocation possible
    /// - [ ] Finality is irreversible
    /// - [ ] Timing attacks on consensus are mitigated
    ///
    /// ## 5. Network Security
    /// - [ ] TLS 1.3 for all connections
    /// - [ ] Certificate validation enforced
    /// - [ ] Rate limiting on all endpoints
    /// - [ ] DoS protection mechanisms in place
    ///
    /// ## 6. State Management
    /// - [ ] Atomic state transitions
    /// - [ ] No state corruption on crash
    /// - [ ] Proper transaction isolation
    ///
    /// ## 7. EVM Security
    /// - [ ] Gas metering correct for all operations
    /// - [ ] Reentrancy protection
    /// - [ ] Stack depth limits enforced
    /// - [ ] Memory expansion limits enforced
    pub const AUDIT_CHECKLIST: &str = include_str!("audit_checklist.md");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_eq() {
        let a = [1, 2, 3, 4, 5];
        let b = [1, 2, 3, 4, 5];
        let c = [1, 2, 3, 4, 6];
        let d = [1, 2, 3];

        assert!(constant_time_eq(&a, &b));
        assert!(!constant_time_eq(&a, &c));
        assert!(!constant_time_eq(&a, &d));
    }

    #[test]
    fn test_constant_time_eq_32() {
        let a = [1u8; 32];
        let b = [1u8; 32];
        let mut c = [1u8; 32];
        c[31] = 2;

        assert!(constant_time_eq_32(&a, &b));
        assert!(!constant_time_eq_32(&a, &c));
    }

    #[test]
    fn test_secure_bytes_zeroize() {
        let data = vec![0xAB; 100];
        let ptr = data.as_ptr();
        let secure = SecureBytes::new(data);

        // Verify data is there
        assert_eq!(secure.len(), 100);
        assert_eq!(secure.as_bytes()[0], 0xAB);

        drop(secure);

        // Note: We can't actually verify the memory is zeroed after drop
        // as the memory may be reallocated. This test verifies the drop runs.
    }

    #[test]
    fn test_validation_message_size() {
        use validation::*;

        assert!(validate_message_size(1000).is_ok());
        assert!(validate_message_size(MAX_MESSAGE_SIZE).is_ok());
        assert!(validate_message_size(MAX_MESSAGE_SIZE + 1).is_err());
    }

    #[test]
    fn test_validation_chain_id() {
        use validation::*;

        assert!(validate_chain_id(&[0u8; 32]).is_ok());
        assert!(validate_chain_id(&[0u8; 31]).is_err());
        assert!(validate_chain_id(&[0u8; 33]).is_err());
    }

    #[test]
    fn test_rate_limiter() {
        use rate_limit::RateLimiter;

        let limiter = RateLimiter::new(10.0, 5);

        // Should allow burst
        for _ in 0..5 {
            assert!(limiter.check("test"));
        }

        // Should be rate limited now
        assert!(!limiter.check("test"));
    }

    #[test]
    fn test_peer_rate_limiter() {
        use rate_limit::PeerRateLimiter;

        let limiter = PeerRateLimiter::new();

        // Small messages should be allowed
        for _ in 0..100 {
            assert!(limiter.check_message("peer1", 100));
        }
    }
}

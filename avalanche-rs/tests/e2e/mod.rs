//! End-to-end tests for avalanche-rs.
//!
//! These tests verify complete node operation including:
//! - Full node bootstrap from genesis
//! - Multi-node consensus reaching finality
//! - Cross-chain atomic transactions
//! - State sync between nodes
//! - API endpoint correctness

pub mod full_node;
pub mod multi_node;
pub mod state_sync;
pub mod cross_chain;

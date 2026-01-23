//! Full node E2E tests.
//!
//! Tests complete node lifecycle from bootstrap to shutdown.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio::time::{timeout, sleep};

use avalanche_ids::{Id, NodeId};

/// Full node test configuration.
#[derive(Debug, Clone)]
pub struct FullNodeConfig {
    /// Node ID.
    pub node_id: NodeId,
    /// Network ID.
    pub network_id: u32,
    /// Listen address.
    pub listen_addr: SocketAddr,
    /// Bootstrap nodes.
    pub bootstrap_nodes: Vec<SocketAddr>,
    /// Data directory (in-memory for tests).
    pub data_dir: Option<String>,
    /// Log level.
    pub log_level: String,
}

impl Default for FullNodeConfig {
    fn default() -> Self {
        Self {
            node_id: NodeId::from_bytes([1; 20]),
            network_id: 12345,
            listen_addr: "127.0.0.1:9650".parse().unwrap(),
            bootstrap_nodes: vec![],
            data_dir: None,
            log_level: "info".to_string(),
        }
    }
}

/// Simulated full node for E2E testing.
pub struct TestFullNode {
    /// Configuration.
    pub config: FullNodeConfig,
    /// Node state.
    state: Arc<RwLock<NodeState>>,
    /// Shutdown signal.
    shutdown_tx: Option<mpsc::Sender<()>>,
}

/// Internal node state.
#[derive(Debug, Default)]
struct NodeState {
    /// Whether the node is running.
    running: bool,
    /// Whether bootstrap is complete.
    bootstrapped: bool,
    /// Current block height per chain.
    heights: HashMap<String, u64>,
    /// Connected peers.
    peers: Vec<NodeId>,
    /// Accepted blocks.
    accepted_blocks: Vec<Id>,
}

impl TestFullNode {
    /// Creates a new test node.
    pub fn new(config: FullNodeConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(NodeState::default())),
            shutdown_tx: None,
        }
    }

    /// Starts the node.
    pub async fn start(&mut self) -> Result<(), NodeError> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        let state = self.state.clone();
        state.write().running = true;

        // Simulate bootstrap process
        tokio::spawn(async move {
            // Simulate bootstrap time
            sleep(Duration::from_millis(100)).await;
            state.write().bootstrapped = true;

            // Initialize chain heights
            {
                let mut s = state.write();
                s.heights.insert("P".to_string(), 0);
                s.heights.insert("X".to_string(), 0);
                s.heights.insert("C".to_string(), 0);
            }

            // Run until shutdown
            let _ = shutdown_rx.recv().await;
            state.write().running = false;
        });

        Ok(())
    }

    /// Stops the node.
    pub async fn stop(&mut self) -> Result<(), NodeError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
        Ok(())
    }

    /// Returns whether the node is running.
    pub fn is_running(&self) -> bool {
        self.state.read().running
    }

    /// Returns whether bootstrap is complete.
    pub fn is_bootstrapped(&self) -> bool {
        self.state.read().bootstrapped
    }

    /// Gets the height of a chain.
    pub fn get_height(&self, chain: &str) -> Option<u64> {
        self.state.read().heights.get(chain).copied()
    }

    /// Adds a peer connection.
    pub fn add_peer(&self, peer_id: NodeId) {
        self.state.write().peers.push(peer_id);
    }

    /// Gets the number of connected peers.
    pub fn peer_count(&self) -> usize {
        self.state.read().peers.len()
    }

    /// Simulates accepting a block.
    pub fn accept_block(&self, chain: &str, block_id: Id) {
        let mut state = self.state.write();
        state.accepted_blocks.push(block_id);
        if let Some(height) = state.heights.get_mut(chain) {
            *height += 1;
        }
    }

    /// Gets the number of accepted blocks.
    pub fn accepted_block_count(&self) -> usize {
        self.state.read().accepted_blocks.len()
    }
}

/// Node error type.
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("failed to start: {0}")]
    StartFailed(String),
    #[error("failed to stop: {0}")]
    StopFailed(String),
    #[error("bootstrap failed: {0}")]
    BootstrapFailed(String),
    #[error("timeout: {0}")]
    Timeout(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_node_lifecycle() {
        let config = FullNodeConfig::default();
        let mut node = TestFullNode::new(config);

        // Start node
        node.start().await.unwrap();
        assert!(node.is_running());

        // Wait for bootstrap
        let result = timeout(Duration::from_secs(5), async {
            while !node.is_bootstrapped() {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        assert!(result.is_ok(), "Bootstrap should complete");
        assert!(node.is_bootstrapped());

        // Check chain heights initialized
        assert_eq!(node.get_height("P"), Some(0));
        assert_eq!(node.get_height("X"), Some(0));
        assert_eq!(node.get_height("C"), Some(0));

        // Stop node
        node.stop().await.unwrap();
        sleep(Duration::from_millis(50)).await;
        assert!(!node.is_running());
    }

    #[tokio::test]
    async fn test_peer_connections() {
        let config = FullNodeConfig::default();
        let mut node = TestFullNode::new(config);

        node.start().await.unwrap();

        // Add peers
        node.add_peer(NodeId::from_bytes([2; 20]));
        node.add_peer(NodeId::from_bytes([3; 20]));
        node.add_peer(NodeId::from_bytes([4; 20]));

        assert_eq!(node.peer_count(), 3);

        node.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_block_acceptance() {
        let config = FullNodeConfig::default();
        let mut node = TestFullNode::new(config);

        node.start().await.unwrap();

        // Wait for bootstrap
        timeout(Duration::from_secs(5), async {
            while !node.is_bootstrapped() {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        // Accept some blocks
        for i in 0..10 {
            let block_id = Id::from_bytes([i; 32]);
            node.accept_block("C", block_id);
        }

        assert_eq!(node.get_height("C"), Some(10));
        assert_eq!(node.accepted_block_count(), 10);

        node.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_multiple_chains() {
        let config = FullNodeConfig::default();
        let mut node = TestFullNode::new(config);

        node.start().await.unwrap();

        timeout(Duration::from_secs(5), async {
            while !node.is_bootstrapped() {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        // Accept blocks on different chains
        node.accept_block("P", Id::from_bytes([1; 32]));
        node.accept_block("P", Id::from_bytes([2; 32]));
        node.accept_block("X", Id::from_bytes([3; 32]));
        node.accept_block("C", Id::from_bytes([4; 32]));
        node.accept_block("C", Id::from_bytes([5; 32]));
        node.accept_block("C", Id::from_bytes([6; 32]));

        assert_eq!(node.get_height("P"), Some(2));
        assert_eq!(node.get_height("X"), Some(1));
        assert_eq!(node.get_height("C"), Some(3));

        node.stop().await.unwrap();
    }

    #[tokio::test]
    async fn test_restart_node() {
        let config = FullNodeConfig::default();
        let mut node = TestFullNode::new(config);

        // First run
        node.start().await.unwrap();
        timeout(Duration::from_secs(5), async {
            while !node.is_bootstrapped() {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        node.stop().await.unwrap();
        sleep(Duration::from_millis(50)).await;

        // Restart
        node.start().await.unwrap();
        assert!(node.is_running());

        timeout(Duration::from_secs(5), async {
            while !node.is_bootstrapped() {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        assert!(node.is_bootstrapped());
        node.stop().await.unwrap();
    }
}

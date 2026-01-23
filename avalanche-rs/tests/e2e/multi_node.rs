//! Multi-node E2E tests.
//!
//! Tests consensus and synchronization between multiple nodes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::time::{timeout, sleep};

use avalanche_ids::{Id, NodeId};

use super::full_node::{FullNodeConfig, TestFullNode, NodeError};

/// Multi-node test cluster.
pub struct TestCluster {
    /// Cluster nodes.
    pub nodes: Vec<TestFullNode>,
    /// Network ID.
    pub network_id: u32,
    /// Base port.
    pub base_port: u16,
}

impl TestCluster {
    /// Creates a new cluster with the specified number of nodes.
    pub fn new(num_nodes: usize, base_port: u16) -> Self {
        let network_id = 12345;
        let nodes = (0..num_nodes)
            .map(|i| {
                let config = FullNodeConfig {
                    node_id: NodeId::from_bytes([i as u8; 20]),
                    network_id,
                    listen_addr: format!("127.0.0.1:{}", base_port + i as u16)
                        .parse()
                        .unwrap(),
                    bootstrap_nodes: vec![],
                    data_dir: None,
                    log_level: "warn".to_string(),
                };
                TestFullNode::new(config)
            })
            .collect();

        Self {
            nodes,
            network_id,
            base_port,
        }
    }

    /// Starts all nodes in the cluster.
    pub async fn start_all(&mut self) -> Result<(), NodeError> {
        for node in &mut self.nodes {
            node.start().await?;
        }
        Ok(())
    }

    /// Stops all nodes in the cluster.
    pub async fn stop_all(&mut self) -> Result<(), NodeError> {
        for node in &mut self.nodes {
            node.stop().await?;
        }
        Ok(())
    }

    /// Waits for all nodes to bootstrap.
    pub async fn wait_for_bootstrap(&self, timeout_duration: Duration) -> Result<(), NodeError> {
        timeout(timeout_duration, async {
            loop {
                if self.nodes.iter().all(|n| n.is_bootstrapped()) {
                    return;
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|_| NodeError::Timeout("bootstrap timeout".to_string()))
    }

    /// Connects all nodes to each other.
    pub fn connect_mesh(&mut self) {
        let node_ids: Vec<NodeId> = self.nodes.iter().map(|n| n.config.node_id).collect();

        for (i, node) in self.nodes.iter().enumerate() {
            for (j, &peer_id) in node_ids.iter().enumerate() {
                if i != j {
                    node.add_peer(peer_id);
                }
            }
        }
    }

    /// Returns the number of running nodes.
    pub fn running_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_running()).count()
    }

    /// Returns the number of bootstrapped nodes.
    pub fn bootstrapped_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_bootstrapped()).count()
    }

    /// Broadcasts a block to all nodes.
    pub fn broadcast_block(&self, chain: &str, block_id: Id) {
        for node in &self.nodes {
            node.accept_block(chain, block_id);
        }
    }

    /// Checks if all nodes have the same height for a chain.
    pub fn check_height_consensus(&self, chain: &str) -> bool {
        let heights: Vec<Option<u64>> = self.nodes.iter().map(|n| n.get_height(chain)).collect();

        if heights.is_empty() {
            return true;
        }

        let first = heights[0];
        heights.iter().all(|h| *h == first)
    }

    /// Gets the maximum height across all nodes for a chain.
    pub fn max_height(&self, chain: &str) -> u64 {
        self.nodes
            .iter()
            .filter_map(|n| n.get_height(chain))
            .max()
            .unwrap_or(0)
    }
}

/// Test result for multi-node scenarios.
#[derive(Debug)]
pub struct TestResult {
    /// Whether the test passed.
    pub passed: bool,
    /// Duration of the test.
    pub duration: Duration,
    /// Final heights per chain.
    pub final_heights: HashMap<String, u64>,
    /// Number of blocks processed.
    pub blocks_processed: usize,
    /// Any error message.
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cluster_creation() {
        let cluster = TestCluster::new(5, 30000);
        assert_eq!(cluster.nodes.len(), 5);
    }

    #[tokio::test]
    async fn test_cluster_startup() {
        let mut cluster = TestCluster::new(3, 30100);

        cluster.start_all().await.unwrap();
        assert_eq!(cluster.running_count(), 3);

        cluster.wait_for_bootstrap(Duration::from_secs(10)).await.unwrap();
        assert_eq!(cluster.bootstrapped_count(), 3);

        cluster.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_mesh_connectivity() {
        let mut cluster = TestCluster::new(5, 30200);

        cluster.start_all().await.unwrap();
        cluster.wait_for_bootstrap(Duration::from_secs(10)).await.unwrap();

        // Connect mesh
        cluster.connect_mesh();

        // Each node should have 4 peers
        for node in &cluster.nodes {
            assert_eq!(node.peer_count(), 4);
        }

        cluster.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_block_broadcast() {
        let mut cluster = TestCluster::new(5, 30300);

        cluster.start_all().await.unwrap();
        cluster.wait_for_bootstrap(Duration::from_secs(10)).await.unwrap();

        // Broadcast blocks
        for i in 0..10 {
            let block_id = Id::from_bytes([i; 32]);
            cluster.broadcast_block("C", block_id);
        }

        // All nodes should have same height
        assert!(cluster.check_height_consensus("C"));
        assert_eq!(cluster.max_height("C"), 10);

        cluster.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_node_failure_recovery() {
        let mut cluster = TestCluster::new(5, 30400);

        cluster.start_all().await.unwrap();
        cluster.wait_for_bootstrap(Duration::from_secs(10)).await.unwrap();

        // Stop one node
        cluster.nodes[2].stop().await.unwrap();
        sleep(Duration::from_millis(50)).await;
        assert_eq!(cluster.running_count(), 4);

        // Cluster should still work
        cluster.broadcast_block("C", Id::from_bytes([1; 32]));

        // Restart the node
        cluster.nodes[2].start().await.unwrap();
        timeout(Duration::from_secs(5), async {
            while !cluster.nodes[2].is_bootstrapped() {
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();

        assert_eq!(cluster.running_count(), 5);

        cluster.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_consensus_under_load() {
        let mut cluster = TestCluster::new(10, 30500);

        cluster.start_all().await.unwrap();
        cluster.wait_for_bootstrap(Duration::from_secs(10)).await.unwrap();
        cluster.connect_mesh();

        // Simulate high throughput
        for i in 0..100 {
            let block_id = Id::from_bytes([i as u8; 32]);
            cluster.broadcast_block("C", block_id);
        }

        // All nodes should agree
        assert!(cluster.check_height_consensus("C"));
        assert_eq!(cluster.max_height("C"), 100);

        cluster.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_multi_chain_consensus() {
        let mut cluster = TestCluster::new(5, 30600);

        cluster.start_all().await.unwrap();
        cluster.wait_for_bootstrap(Duration::from_secs(10)).await.unwrap();

        // Broadcast blocks to different chains
        for i in 0..20 {
            cluster.broadcast_block("P", Id::from_bytes([i; 32]));
        }
        for i in 0..30 {
            cluster.broadcast_block("X", Id::from_bytes([(100 + i) as u8; 32]));
        }
        for i in 0..50 {
            cluster.broadcast_block("C", Id::from_bytes([(200 + i) as u8; 32]));
        }

        // Check consensus on all chains
        assert!(cluster.check_height_consensus("P"));
        assert!(cluster.check_height_consensus("X"));
        assert!(cluster.check_height_consensus("C"));

        assert_eq!(cluster.max_height("P"), 20);
        assert_eq!(cluster.max_height("X"), 30);
        assert_eq!(cluster.max_height("C"), 50);

        cluster.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_network_partition() {
        let mut cluster = TestCluster::new(6, 30700);

        cluster.start_all().await.unwrap();
        cluster.wait_for_bootstrap(Duration::from_secs(10)).await.unwrap();

        // Simulate partition: first 3 nodes and last 3 nodes
        // Only broadcast to first partition
        for i in 0..5 {
            let block_id = Id::from_bytes([i; 32]);
            for node in cluster.nodes.iter().take(3) {
                node.accept_block("C", block_id);
            }
        }

        // First partition has blocks
        assert_eq!(cluster.nodes[0].get_height("C"), Some(5));
        assert_eq!(cluster.nodes[1].get_height("C"), Some(5));
        assert_eq!(cluster.nodes[2].get_height("C"), Some(5));

        // Second partition doesn't
        assert_eq!(cluster.nodes[3].get_height("C"), Some(0));
        assert_eq!(cluster.nodes[4].get_height("C"), Some(0));
        assert_eq!(cluster.nodes[5].get_height("C"), Some(0));

        // Heal partition - sync all nodes
        for i in 0..5 {
            let block_id = Id::from_bytes([i; 32]);
            for node in cluster.nodes.iter().skip(3) {
                node.accept_block("C", block_id);
            }
        }

        // Now all should agree
        assert!(cluster.check_height_consensus("C"));

        cluster.stop_all().await.unwrap();
    }

    /// Stress test with many nodes.
    #[tokio::test]
    async fn test_large_cluster() {
        let mut cluster = TestCluster::new(20, 31000);

        cluster.start_all().await.unwrap();
        cluster.wait_for_bootstrap(Duration::from_secs(30)).await.unwrap();

        // All nodes should bootstrap
        assert_eq!(cluster.bootstrapped_count(), 20);

        // Broadcast blocks
        for i in 0..50 {
            cluster.broadcast_block("C", Id::from_bytes([i as u8; 32]));
        }

        assert!(cluster.check_height_consensus("C"));
        assert_eq!(cluster.max_height("C"), 50);

        cluster.stop_all().await.unwrap();
    }
}

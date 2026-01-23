//! Cross-chain atomic transaction E2E tests.
//!
//! Tests atomic transactions between P/X/C chains.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::time::sleep;

use avalanche_ids::Id;

/// Cross-chain transaction status.
#[derive(Debug, Clone, PartialEq)]
pub enum AtomicTxStatus {
    /// Transaction is pending.
    Pending,
    /// Transaction exported from source chain.
    Exported,
    /// Transaction imported to destination chain.
    Imported,
    /// Transaction completed successfully.
    Completed,
    /// Transaction failed.
    Failed(String),
}

/// Atomic transaction for cross-chain transfers.
#[derive(Debug, Clone)]
pub struct AtomicTransaction {
    /// Transaction ID.
    pub tx_id: Id,
    /// Source chain.
    pub source_chain: String,
    /// Destination chain.
    pub dest_chain: String,
    /// Amount in nAVAX.
    pub amount: u64,
    /// Transaction status.
    pub status: AtomicTxStatus,
    /// Export block height.
    pub export_height: Option<u64>,
    /// Import block height.
    pub import_height: Option<u64>,
}

impl AtomicTransaction {
    /// Creates a new atomic transaction.
    pub fn new(tx_id: Id, source: &str, dest: &str, amount: u64) -> Self {
        Self {
            tx_id,
            source_chain: source.to_string(),
            dest_chain: dest.to_string(),
            amount,
            status: AtomicTxStatus::Pending,
            export_height: None,
            import_height: None,
        }
    }
}

/// Simulated chain for cross-chain testing.
pub struct TestChain {
    /// Chain ID (P, X, or C).
    pub chain_id: String,
    /// Current block height.
    height: Arc<RwLock<u64>>,
    /// Exported UTXOs (tx_id -> amount).
    exported_utxos: Arc<RwLock<HashMap<Id, u64>>>,
    /// Imported UTXOs.
    imported_utxos: Arc<RwLock<HashMap<Id, u64>>>,
    /// Balance.
    balance: Arc<RwLock<u64>>,
}

impl TestChain {
    /// Creates a new test chain.
    pub fn new(chain_id: &str, initial_balance: u64) -> Self {
        Self {
            chain_id: chain_id.to_string(),
            height: Arc::new(RwLock::new(0)),
            exported_utxos: Arc::new(RwLock::new(HashMap::new())),
            imported_utxos: Arc::new(RwLock::new(HashMap::new())),
            balance: Arc::new(RwLock::new(initial_balance)),
        }
    }

    /// Gets the current height.
    pub fn height(&self) -> u64 {
        *self.height.read()
    }

    /// Gets the current balance.
    pub fn balance(&self) -> u64 {
        *self.balance.read()
    }

    /// Advances the block height.
    pub fn advance(&self) {
        *self.height.write() += 1;
    }

    /// Exports an atomic UTXO.
    pub fn export(&self, tx_id: Id, amount: u64) -> Result<u64, CrossChainError> {
        let mut balance = self.balance.write();
        if *balance < amount {
            return Err(CrossChainError::InsufficientFunds {
                available: *balance,
                required: amount,
            });
        }

        *balance -= amount;
        self.exported_utxos.write().insert(tx_id, amount);
        self.advance();

        Ok(self.height())
    }

    /// Imports an atomic UTXO.
    pub fn import(&self, tx_id: Id, amount: u64) -> Result<u64, CrossChainError> {
        // Check if UTXO already imported
        if self.imported_utxos.read().contains_key(&tx_id) {
            return Err(CrossChainError::AlreadyImported(tx_id));
        }

        *self.balance.write() += amount;
        self.imported_utxos.write().insert(tx_id, amount);
        self.advance();

        Ok(self.height())
    }

    /// Gets the number of exported UTXOs.
    pub fn export_count(&self) -> usize {
        self.exported_utxos.read().len()
    }

    /// Gets the number of imported UTXOs.
    pub fn import_count(&self) -> usize {
        self.imported_utxos.read().len()
    }
}

/// Cross-chain coordinator for atomic transactions.
pub struct CrossChainCoordinator {
    /// P-Chain.
    pub p_chain: TestChain,
    /// X-Chain.
    pub x_chain: TestChain,
    /// C-Chain.
    pub c_chain: TestChain,
    /// Pending transactions.
    pending_txs: Arc<RwLock<Vec<AtomicTransaction>>>,
    /// Completed transactions.
    completed_txs: Arc<RwLock<Vec<AtomicTransaction>>>,
}

impl CrossChainCoordinator {
    /// Creates a new coordinator with initial balances.
    pub fn new(p_balance: u64, x_balance: u64, c_balance: u64) -> Self {
        Self {
            p_chain: TestChain::new("P", p_balance),
            x_chain: TestChain::new("X", x_balance),
            c_chain: TestChain::new("C", c_balance),
            pending_txs: Arc::new(RwLock::new(Vec::new())),
            completed_txs: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Gets a chain by ID.
    pub fn get_chain(&self, chain_id: &str) -> Option<&TestChain> {
        match chain_id {
            "P" => Some(&self.p_chain),
            "X" => Some(&self.x_chain),
            "C" => Some(&self.c_chain),
            _ => None,
        }
    }

    /// Submits an atomic transaction.
    pub fn submit_atomic_tx(&self, tx: AtomicTransaction) -> Id {
        let tx_id = tx.tx_id;
        self.pending_txs.write().push(tx);
        tx_id
    }

    /// Processes pending transactions.
    pub async fn process_pending(&self) -> Vec<Id> {
        let mut completed = Vec::new();
        let mut pending = self.pending_txs.write();
        let mut to_remove = Vec::new();

        for (idx, tx) in pending.iter_mut().enumerate() {
            match self.process_tx(tx).await {
                Ok(()) => {
                    completed.push(tx.tx_id);
                    to_remove.push(idx);
                }
                Err(_) => {
                    // Keep in pending for retry
                }
            }
        }

        // Move completed to completed list
        for &idx in to_remove.iter().rev() {
            let tx = pending.remove(idx);
            self.completed_txs.write().push(tx);
        }

        completed
    }

    /// Processes a single atomic transaction.
    async fn process_tx(&self, tx: &mut AtomicTransaction) -> Result<(), CrossChainError> {
        let source = self
            .get_chain(&tx.source_chain)
            .ok_or_else(|| CrossChainError::UnknownChain(tx.source_chain.clone()))?;
        let dest = self
            .get_chain(&tx.dest_chain)
            .ok_or_else(|| CrossChainError::UnknownChain(tx.dest_chain.clone()))?;

        // Export from source
        if tx.status == AtomicTxStatus::Pending {
            let height = source.export(tx.tx_id, tx.amount)?;
            tx.export_height = Some(height);
            tx.status = AtomicTxStatus::Exported;
        }

        // Simulate processing time
        sleep(Duration::from_millis(1)).await;

        // Import to destination
        if tx.status == AtomicTxStatus::Exported {
            let height = dest.import(tx.tx_id, tx.amount)?;
            tx.import_height = Some(height);
            tx.status = AtomicTxStatus::Imported;
        }

        tx.status = AtomicTxStatus::Completed;
        Ok(())
    }

    /// Gets the number of pending transactions.
    pub fn pending_count(&self) -> usize {
        self.pending_txs.read().len()
    }

    /// Gets the number of completed transactions.
    pub fn completed_count(&self) -> usize {
        self.completed_txs.read().len()
    }

    /// Gets total balance across all chains.
    pub fn total_balance(&self) -> u64 {
        self.p_chain.balance() + self.x_chain.balance() + self.c_chain.balance()
    }
}

/// Cross-chain error type.
#[derive(Debug, thiserror::Error)]
pub enum CrossChainError {
    #[error("unknown chain: {0}")]
    UnknownChain(String),
    #[error("insufficient funds: have {available}, need {required}")]
    InsufficientFunds { available: u64, required: u64 },
    #[error("UTXO already imported: {0}")]
    AlreadyImported(Id),
    #[error("transaction failed: {0}")]
    TransactionFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_creation() {
        let chain = TestChain::new("C", 1000);
        assert_eq!(chain.chain_id, "C");
        assert_eq!(chain.balance(), 1000);
        assert_eq!(chain.height(), 0);
    }

    #[test]
    fn test_chain_export() {
        let chain = TestChain::new("X", 1000);
        let tx_id = Id::from_bytes([1; 32]);

        let height = chain.export(tx_id, 500).unwrap();
        assert_eq!(height, 1);
        assert_eq!(chain.balance(), 500);
        assert_eq!(chain.export_count(), 1);
    }

    #[test]
    fn test_chain_import() {
        let chain = TestChain::new("C", 1000);
        let tx_id = Id::from_bytes([1; 32]);

        let height = chain.import(tx_id, 500).unwrap();
        assert_eq!(height, 1);
        assert_eq!(chain.balance(), 1500);
        assert_eq!(chain.import_count(), 1);
    }

    #[test]
    fn test_insufficient_funds() {
        let chain = TestChain::new("X", 100);
        let tx_id = Id::from_bytes([1; 32]);

        let result = chain.export(tx_id, 500);
        assert!(result.is_err());
    }

    #[test]
    fn test_double_import() {
        let chain = TestChain::new("C", 1000);
        let tx_id = Id::from_bytes([1; 32]);

        chain.import(tx_id, 500).unwrap();
        let result = chain.import(tx_id, 500);
        assert!(result.is_err());
    }

    #[test]
    fn test_coordinator_creation() {
        let coord = CrossChainCoordinator::new(1000, 2000, 3000);
        assert_eq!(coord.total_balance(), 6000);
    }

    #[tokio::test]
    async fn test_atomic_x_to_c() {
        let coord = CrossChainCoordinator::new(0, 1000, 0);
        let tx_id = Id::from_bytes([1; 32]);

        let tx = AtomicTransaction::new(tx_id, "X", "C", 500);
        coord.submit_atomic_tx(tx);

        coord.process_pending().await;

        assert_eq!(coord.x_chain.balance(), 500);
        assert_eq!(coord.c_chain.balance(), 500);
        assert_eq!(coord.completed_count(), 1);
        assert_eq!(coord.total_balance(), 1000); // Conservation of value
    }

    #[tokio::test]
    async fn test_atomic_p_to_x() {
        let coord = CrossChainCoordinator::new(1000, 0, 0);
        let tx_id = Id::from_bytes([2; 32]);

        let tx = AtomicTransaction::new(tx_id, "P", "X", 750);
        coord.submit_atomic_tx(tx);

        coord.process_pending().await;

        assert_eq!(coord.p_chain.balance(), 250);
        assert_eq!(coord.x_chain.balance(), 750);
        assert_eq!(coord.total_balance(), 1000);
    }

    #[tokio::test]
    async fn test_multiple_atomic_txs() {
        let coord = CrossChainCoordinator::new(1000, 1000, 1000);

        // Submit multiple transactions
        for i in 0..10 {
            let tx_id = Id::from_bytes([i; 32]);
            let tx = AtomicTransaction::new(tx_id, "X", "C", 50);
            coord.submit_atomic_tx(tx);
        }

        coord.process_pending().await;

        assert_eq!(coord.x_chain.balance(), 500); // 1000 - 10*50
        assert_eq!(coord.c_chain.balance(), 1500); // 1000 + 10*50
        assert_eq!(coord.completed_count(), 10);
        assert_eq!(coord.total_balance(), 3000); // Conserved
    }

    #[tokio::test]
    async fn test_cross_chain_round_trip() {
        let coord = CrossChainCoordinator::new(0, 1000, 0);

        // X -> C
        let tx1 = AtomicTransaction::new(Id::from_bytes([1; 32]), "X", "C", 500);
        coord.submit_atomic_tx(tx1);
        coord.process_pending().await;

        assert_eq!(coord.x_chain.balance(), 500);
        assert_eq!(coord.c_chain.balance(), 500);

        // C -> X
        let tx2 = AtomicTransaction::new(Id::from_bytes([2; 32]), "C", "X", 300);
        coord.submit_atomic_tx(tx2);
        coord.process_pending().await;

        assert_eq!(coord.x_chain.balance(), 800);
        assert_eq!(coord.c_chain.balance(), 200);
        assert_eq!(coord.total_balance(), 1000);
    }

    #[tokio::test]
    async fn test_three_chain_transfers() {
        let coord = CrossChainCoordinator::new(1000, 0, 0);

        // P -> X
        let tx1 = AtomicTransaction::new(Id::from_bytes([1; 32]), "P", "X", 500);
        coord.submit_atomic_tx(tx1);
        coord.process_pending().await;

        // X -> C
        let tx2 = AtomicTransaction::new(Id::from_bytes([2; 32]), "X", "C", 300);
        coord.submit_atomic_tx(tx2);
        coord.process_pending().await;

        // C -> P
        let tx3 = AtomicTransaction::new(Id::from_bytes([3; 32]), "C", "P", 100);
        coord.submit_atomic_tx(tx3);
        coord.process_pending().await;

        assert_eq!(coord.p_chain.balance(), 600); // 1000 - 500 + 100
        assert_eq!(coord.x_chain.balance(), 200); // 0 + 500 - 300
        assert_eq!(coord.c_chain.balance(), 200); // 0 + 300 - 100
        assert_eq!(coord.total_balance(), 1000);
    }

    #[tokio::test]
    async fn test_atomic_tx_heights() {
        let coord = CrossChainCoordinator::new(0, 1000, 0);
        let tx_id = Id::from_bytes([1; 32]);

        let tx = AtomicTransaction::new(tx_id, "X", "C", 500);
        coord.submit_atomic_tx(tx);
        coord.process_pending().await;

        // Both chains should have advanced
        assert!(coord.x_chain.height() > 0);
        assert!(coord.c_chain.height() > 0);
    }

    /// Stress test with many transactions.
    #[tokio::test]
    async fn test_high_throughput() {
        let coord = CrossChainCoordinator::new(100000, 100000, 100000);
        let initial_total = coord.total_balance();

        // Submit 100 transactions
        for i in 0..100u8 {
            let tx_id = Id::from_bytes([i; 32]);
            let source = match i % 3 {
                0 => "P",
                1 => "X",
                _ => "C",
            };
            let dest = match (i + 1) % 3 {
                0 => "P",
                1 => "X",
                _ => "C",
            };
            let tx = AtomicTransaction::new(tx_id, source, dest, 100);
            coord.submit_atomic_tx(tx);
        }

        coord.process_pending().await;

        assert_eq!(coord.completed_count(), 100);
        assert_eq!(coord.total_balance(), initial_total); // Conservation
    }
}

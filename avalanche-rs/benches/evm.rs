//! EVM benchmarks.
//!
//! Benchmarks for EVM execution, state operations, and transaction processing.

use std::collections::HashMap;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use sha3::{Digest, Keccak256};

/// Simple EVM state for benchmarking.
struct BenchState {
    accounts: HashMap<[u8; 20], BenchAccount>,
    storage: HashMap<([u8; 20], [u8; 32]), [u8; 32]>,
}

/// Account state.
#[derive(Clone, Default)]
struct BenchAccount {
    nonce: u64,
    balance: u128,
    code_hash: [u8; 32],
    storage_root: [u8; 32],
}

impl BenchState {
    fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            storage: HashMap::new(),
        }
    }

    fn get_account(&self, addr: &[u8; 20]) -> Option<&BenchAccount> {
        self.accounts.get(addr)
    }

    fn set_account(&mut self, addr: [u8; 20], account: BenchAccount) {
        self.accounts.insert(addr, account);
    }

    fn get_storage(&self, addr: &[u8; 20], slot: &[u8; 32]) -> [u8; 32] {
        self.storage.get(&(*addr, *slot)).copied().unwrap_or([0u8; 32])
    }

    fn set_storage(&mut self, addr: [u8; 20], slot: [u8; 32], value: [u8; 32]) {
        self.storage.insert((addr, slot), value);
    }

    fn account_count(&self) -> usize {
        self.accounts.len()
    }

    fn storage_count(&self) -> usize {
        self.storage.len()
    }
}

/// Simple transaction for benchmarking.
struct BenchTx {
    from: [u8; 20],
    to: Option<[u8; 20]>,
    value: u128,
    data: Vec<u8>,
    gas_limit: u64,
    gas_price: u128,
    nonce: u64,
}

impl BenchTx {
    fn transfer(from: [u8; 20], to: [u8; 20], value: u128) -> Self {
        Self {
            from,
            to: Some(to),
            value,
            data: Vec::new(),
            gas_limit: 21000,
            gas_price: 25_000_000_000,
            nonce: 0,
        }
    }

    fn contract_call(from: [u8; 20], to: [u8; 20], data: Vec<u8>, gas_limit: u64) -> Self {
        Self {
            from,
            to: Some(to),
            value: 0,
            data,
            gas_limit,
            gas_price: 25_000_000_000,
            nonce: 0,
        }
    }
}

/// Benchmark account lookups.
fn bench_account_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("evm_account_lookup");

    for num_accounts in [100, 1000, 10000, 100000].iter() {
        let mut state = BenchState::new();

        // Populate accounts
        for i in 0..*num_accounts {
            let mut addr = [0u8; 20];
            addr[0..8].copy_from_slice(&(i as u64).to_be_bytes());

            state.set_account(addr, BenchAccount {
                nonce: i as u64,
                balance: 1_000_000_000_000_000_000,
                code_hash: [0u8; 32],
                storage_root: [0u8; 32],
            });
        }

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            &state,
            |b, state| {
                // Look up a random account
                let mut addr = [0u8; 20];
                addr[0..8].copy_from_slice(&((*num_accounts / 2) as u64).to_be_bytes());

                b.iter(|| {
                    black_box(state.get_account(&addr))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark storage lookups.
fn bench_storage_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("evm_storage_lookup");

    for num_slots in [100, 1000, 10000].iter() {
        let mut state = BenchState::new();
        let addr = [1u8; 20];

        // Populate storage
        for i in 0..*num_slots {
            let mut slot = [0u8; 32];
            slot[0..8].copy_from_slice(&(i as u64).to_be_bytes());

            let mut value = [0u8; 32];
            value[24..32].copy_from_slice(&(i as u64).to_be_bytes());

            state.set_storage(addr, slot, value);
        }

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_slots),
            &state,
            |b, state| {
                let mut slot = [0u8; 32];
                slot[0..8].copy_from_slice(&((*num_slots / 2) as u64).to_be_bytes());

                b.iter(|| {
                    black_box(state.get_storage(&addr, &slot))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark state modifications.
fn bench_state_modification(c: &mut Criterion) {
    let mut group = c.benchmark_group("evm_state_modification");

    for batch_size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut state = BenchState::new();

                    for i in 0..size {
                        let mut addr = [0u8; 20];
                        addr[0..8].copy_from_slice(&(i as u64).to_be_bytes());

                        state.set_account(addr, BenchAccount {
                            nonce: i as u64,
                            balance: 1_000_000_000_000_000_000,
                            ..Default::default()
                        });

                        // Add some storage
                        let slot = [i as u8; 32];
                        let value = [(i + 1) as u8; 32];
                        state.set_storage(addr, slot, value);
                    }

                    black_box(state.account_count())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark simple transfer transactions.
fn bench_transfer_tx(c: &mut Criterion) {
    c.bench_function("evm_transfer_tx", |b| {
        let mut state = BenchState::new();

        // Setup sender with balance
        let sender = [1u8; 20];
        let receiver = [2u8; 20];

        state.set_account(sender, BenchAccount {
            nonce: 0,
            balance: 10_000_000_000_000_000_000,
            ..Default::default()
        });

        state.set_account(receiver, BenchAccount::default());

        b.iter(|| {
            let tx = BenchTx::transfer(sender, receiver, 1_000_000_000_000_000_000);

            // Simulate transfer execution
            let mut sender_acc = state.get_account(&sender).unwrap().clone();
            let mut receiver_acc = state.get_account(&receiver).unwrap().clone();

            // Check balance
            if sender_acc.balance >= tx.value {
                sender_acc.balance -= tx.value;
                sender_acc.nonce += 1;
                receiver_acc.balance += tx.value;

                state.set_account(sender, sender_acc);
                state.set_account(receiver, receiver_acc);
            }

            black_box(state.get_account(&sender).unwrap().nonce)
        });
    });
}

/// Benchmark contract call simulation.
fn bench_contract_call(c: &mut Criterion) {
    let mut group = c.benchmark_group("evm_contract_call");

    // Simulate different call data sizes
    for data_size in [0, 36, 100, 1000].iter() {
        let data = vec![0xABu8; *data_size];

        group.throughput(Throughput::Bytes(*data_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(data_size),
            &data,
            |b, data| {
                let sender = [1u8; 20];
                let contract = [2u8; 20];

                b.iter(|| {
                    let tx = BenchTx::contract_call(sender, contract, data.clone(), 100000);

                    // Simulate function selector extraction
                    let selector = if tx.data.len() >= 4 {
                        let mut sel = [0u8; 4];
                        sel.copy_from_slice(&tx.data[..4]);
                        sel
                    } else {
                        [0u8; 4]
                    };

                    // Simulate ABI decoding overhead
                    let _params: Vec<[u8; 32]> = tx.data[4..]
                        .chunks(32)
                        .map(|chunk| {
                            let mut word = [0u8; 32];
                            let len = chunk.len().min(32);
                            word[..len].copy_from_slice(&chunk[..len]);
                            word
                        })
                        .collect();

                    black_box(selector)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark gas computation.
fn bench_gas_computation(c: &mut Criterion) {
    c.bench_function("evm_gas_computation", |b| {
        b.iter(|| {
            // Simulate gas computation for various operations
            let mut gas_used = 0u64;

            // Base tx cost
            gas_used += 21000;

            // Calldata cost (16 gas per non-zero byte, 4 per zero)
            let data = vec![0xABu8; 100];
            for &byte in &data {
                gas_used += if byte == 0 { 4 } else { 16 };
            }

            // Storage operations
            gas_used += 20000; // SSTORE (cold)
            gas_used += 100; // SLOAD (warm)

            // Memory expansion
            let mem_size = 1024u64;
            let memory_cost = (mem_size * mem_size) / 512 + 3 * mem_size;
            gas_used += memory_cost;

            black_box(gas_used)
        });
    });
}

/// Benchmark receipt creation.
fn bench_receipt_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("evm_receipt_creation");

    for num_logs in [0, 1, 5, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_logs),
            num_logs,
            |b, &n| {
                b.iter(|| {
                    // Simulate receipt with logs
                    let logs: Vec<([u8; 20], Vec<[u8; 32]>, Vec<u8>)> = (0..n)
                        .map(|i| {
                            let addr = [i as u8; 20];
                            let topics = vec![[i as u8; 32]; 3];
                            let data = vec![i as u8; 64];
                            (addr, topics, data)
                        })
                        .collect();

                    // Compute bloom filter
                    let mut bloom = [0u8; 256];
                    for (addr, topics, _) in &logs {
                        add_to_bloom(&mut bloom, addr);
                        for topic in topics {
                            add_to_bloom(&mut bloom, topic);
                        }
                    }

                    black_box(bloom)
                });
            },
        );
    }

    group.finish();
}

/// Add data to bloom filter.
fn add_to_bloom(bloom: &mut [u8; 256], data: &[u8]) {
    let hash = Keccak256::digest(data);

    for i in 0..3 {
        let bit = ((hash[i * 2] as usize) << 8 | hash[i * 2 + 1] as usize) & 0x7FF;
        bloom[bit / 8] |= 1 << (7 - bit % 8);
    }
}

/// Benchmark block building simulation.
fn bench_block_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("evm_block_building");

    for num_txs in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*num_txs as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_txs),
            num_txs,
            |b, &n| {
                b.iter(|| {
                    let mut state = BenchState::new();
                    let mut gas_used = 0u64;
                    let gas_limit = 8_000_000u64;
                    let mut tx_hashes = Vec::new();

                    // Pre-populate some accounts
                    for i in 0..100 {
                        let mut addr = [0u8; 20];
                        addr[0..8].copy_from_slice(&(i as u64).to_be_bytes());
                        state.set_account(addr, BenchAccount {
                            balance: 1_000_000_000_000_000_000_000,
                            ..Default::default()
                        });
                    }

                    // Process transactions
                    for i in 0..n {
                        let tx_gas = 21000;
                        if gas_used + tx_gas > gas_limit {
                            break;
                        }

                        // Simulate tx hash
                        let mut tx_data = [0u8; 32];
                        tx_data[0..8].copy_from_slice(&(i as u64).to_be_bytes());
                        let tx_hash = Keccak256::digest(&tx_data);
                        tx_hashes.push(tx_hash);

                        gas_used += tx_gas;
                    }

                    black_box((gas_used, tx_hashes.len()))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark state root computation.
fn bench_state_root(c: &mut Criterion) {
    let mut group = c.benchmark_group("evm_state_root");

    for num_accounts in [10, 100, 1000].iter() {
        let mut state = BenchState::new();

        for i in 0..*num_accounts {
            let mut addr = [0u8; 20];
            addr[0..8].copy_from_slice(&(i as u64).to_be_bytes());
            state.set_account(addr, BenchAccount {
                nonce: i as u64,
                balance: 1_000_000_000_000_000_000,
                ..Default::default()
            });
        }

        group.throughput(Throughput::Elements(*num_accounts as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_accounts),
            &state,
            |b, state| {
                b.iter(|| {
                    // Simulate simplified state root computation
                    let mut combined = Vec::new();

                    for (addr, account) in &state.accounts {
                        combined.extend_from_slice(addr);
                        combined.extend_from_slice(&account.nonce.to_be_bytes());
                        combined.extend_from_slice(&account.balance.to_be_bytes());
                    }

                    let root = Keccak256::digest(&combined);
                    black_box(root)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_account_lookup,
    bench_storage_lookup,
    bench_state_modification,
    bench_transfer_tx,
    bench_contract_call,
    bench_gas_computation,
    bench_receipt_creation,
    bench_block_building,
    bench_state_root,
);

criterion_main!(benches);

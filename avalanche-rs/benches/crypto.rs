//! Cryptography benchmarks.
//!
//! Benchmarks for signature operations, hashing, and key generation.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use sha2::{Digest, Sha256};
use sha3::Keccak256;

/// Benchmark SHA256 hashing at various input sizes.
fn bench_sha256(c: &mut Criterion) {
    let mut group = c.benchmark_group("sha256");

    for size in [32, 256, 1024, 4096, 65536].iter() {
        let data = vec![0u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &data,
            |b, data| {
                b.iter(|| {
                    let hash = Sha256::digest(black_box(data));
                    black_box(hash)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Keccak256 hashing (used in EVM).
fn bench_keccak256(c: &mut Criterion) {
    let mut group = c.benchmark_group("keccak256");

    for size in [32, 256, 1024, 4096, 65536].iter() {
        let data = vec![0u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &data,
            |b, data| {
                b.iter(|| {
                    let hash = Keccak256::digest(black_box(data));
                    black_box(hash)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark double SHA256 (used in Bitcoin-style hashing).
fn bench_double_sha256(c: &mut Criterion) {
    let mut group = c.benchmark_group("double_sha256");

    for size in [32, 256, 1024].iter() {
        let data = vec![0u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &data,
            |b, data| {
                b.iter(|| {
                    let hash1 = Sha256::digest(black_box(data));
                    let hash2 = Sha256::digest(&hash1);
                    black_box(hash2)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark address derivation from public key.
fn bench_address_derivation(c: &mut Criterion) {
    c.bench_function("address_derivation", |b| {
        // Mock 65-byte uncompressed public key
        let pubkey = [0u8; 65];

        b.iter(|| {
            // Ethereum-style address: keccak256(pubkey[1:])[12:]
            let hash = Keccak256::digest(&pubkey[1..]);
            let address: [u8; 20] = hash[12..32].try_into().unwrap();
            black_box(address)
        });
    });
}

/// Benchmark Merkle root computation.
fn bench_merkle_root(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle_root");

    for num_leaves in [4, 16, 64, 256, 1024].iter() {
        // Create leaves
        let leaves: Vec<[u8; 32]> = (0..*num_leaves)
            .map(|i| {
                let mut hash = [0u8; 32];
                hash[0..8].copy_from_slice(&(i as u64).to_be_bytes());
                hash
            })
            .collect();

        group.throughput(Throughput::Elements(*num_leaves as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_leaves),
            &leaves,
            |b, leaves| {
                b.iter(|| {
                    compute_merkle_root(black_box(leaves.clone()))
                });
            },
        );
    }

    group.finish();
}

/// Compute Merkle root from leaves.
fn compute_merkle_root(leaves: Vec<[u8; 32]>) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }

    let mut current = leaves;

    while current.len() > 1 {
        let mut next = Vec::new();

        for i in (0..current.len()).step_by(2) {
            let left = current[i];
            let right = if i + 1 < current.len() {
                current[i + 1]
            } else {
                current[i] // Duplicate last if odd
            };

            let mut combined = [0u8; 64];
            combined[..32].copy_from_slice(&left);
            combined[32..].copy_from_slice(&right);

            let hash = Sha256::digest(&combined);
            next.push(hash.into());
        }

        current = next;
    }

    current[0]
}

/// Benchmark batch hashing (simulating block verification).
fn bench_batch_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_hashing");

    for batch_size in [10, 100, 1000].iter() {
        let items: Vec<Vec<u8>> = (0..*batch_size)
            .map(|i| {
                let mut data = vec![0u8; 256];
                data[0..8].copy_from_slice(&(i as u64).to_be_bytes());
                data
            })
            .collect();

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &items,
            |b, items| {
                b.iter(|| {
                    let hashes: Vec<_> = items
                        .iter()
                        .map(|item| Sha256::digest(item))
                        .collect();
                    black_box(hashes)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark RIPEMD160 (used in P2PKH addresses).
fn bench_ripemd160(c: &mut Criterion) {
    use ripemd::Ripemd160;

    let mut group = c.benchmark_group("ripemd160");

    for size in [20, 32, 256].iter() {
        let data = vec![0u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &data,
            |b, data| {
                b.iter(|| {
                    let hash = Ripemd160::digest(black_box(data));
                    black_box(hash)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark Hash160 (SHA256 + RIPEMD160).
fn bench_hash160(c: &mut Criterion) {
    use ripemd::Ripemd160;

    c.bench_function("hash160", |b| {
        let data = [0u8; 33]; // Compressed public key

        b.iter(|| {
            let sha = Sha256::digest(black_box(&data));
            let hash = Ripemd160::digest(&sha);
            black_box(hash)
        });
    });
}

/// Benchmark CB58 encoding (used for Avalanche addresses).
fn bench_cb58_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("cb58_encode");

    for size in [20, 32, 64].iter() {
        let data = vec![0xABu8; *size];

        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &data,
            |b, data| {
                b.iter(|| {
                    cb58_encode(black_box(data))
                });
            },
        );
    }

    group.finish();
}

/// Simple CB58 encode (Base58Check).
fn cb58_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    // Add 4-byte checksum
    let checksum = Sha256::digest(Sha256::digest(data));
    let mut bytes = Vec::with_capacity(data.len() + 4);
    bytes.extend_from_slice(data);
    bytes.extend_from_slice(&checksum[..4]);

    // Count leading zeros
    let leading_zeros = bytes.iter().take_while(|&&b| b == 0).count();

    // Convert to base58
    let mut result = String::new();

    while !bytes.is_empty() && bytes.iter().any(|&b| b != 0) {
        let mut remainder = 0u32;
        let mut new_bytes = Vec::new();

        for &byte in &bytes {
            let value = (remainder << 8) + byte as u32;
            let digit = value / 58;
            remainder = value % 58;

            if !new_bytes.is_empty() || digit > 0 {
                new_bytes.push(digit as u8);
            }
        }

        result.push(ALPHABET[remainder as usize] as char);
        bytes = new_bytes;
    }

    // Add leading '1's for leading zeros
    for _ in 0..leading_zeros {
        result.push('1');
    }

    result.chars().rev().collect()
}

criterion_group!(
    benches,
    bench_sha256,
    bench_keccak256,
    bench_double_sha256,
    bench_address_derivation,
    bench_merkle_root,
    bench_batch_hashing,
    bench_ripemd160,
    bench_hash160,
    bench_cb58_encode,
);

criterion_main!(benches);

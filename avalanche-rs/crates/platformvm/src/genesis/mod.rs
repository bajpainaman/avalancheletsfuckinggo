//! Genesis configuration modules.
//!
//! Contains core genesis types and mainnet-specific parsing.

pub mod core;
pub mod mainnet;

// Re-export core types
pub use core::{
    Genesis, Allocation, GenesisChain, GenesisError,
    defaults,
};

// Re-export mainnet parser
pub use mainnet::{
    MainnetGenesisRaw, ParsedMainnetGenesis, ParsedStaker, ParsedAllocation,
    RawAllocation, RawStaker, UnlockScheduleEntry, GenesisParseError,
    constants as mainnet_constants,
};

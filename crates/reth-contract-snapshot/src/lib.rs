//! Root-pinned Ethereum account snapshots read directly from a live Reth datadir.

mod snapshot;

pub mod provider;

pub use snapshot::{SnapshotError, key_successor, validate_bytecode, write_accounts};
pub use snapshot_format::{
    ContractStateSource, PageProgress, SnapshotAccount, SnapshotAnchor, SnapshotStats, StoragePage,
    StorageRangeEnd,
};

/// Default byte budget for one Reth storage-range response.
pub const DEFAULT_PAGE_BYTES: usize = 4 * 1024 * 1024;

#[cfg(test)]
mod tests;

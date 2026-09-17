//! Portable schema and validation for multi-account contract-state snapshots.

mod format;
mod source;

pub use format::{
    AccountRecord, AccountSnapshot, BlockContext, BlockHashes, EvmHardfork, ExecutionContext,
    PageProgress, Snapshot, SnapshotAccount, SnapshotAnchor, SnapshotFormatError, SnapshotStats,
    StorageEntries, StoragePage, StorageRangeEnd, StorageRootBuilder, VERSION,
};
pub use source::ContractStateSource;

use super::{AccountSnapshot, BlockContext, BlockHashes, ExecutionContext};
use alloy_primitives::{Address, B256, Bytes, U256};

/// Execution and block identity shared by every account in one batch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotAnchor {
    pub execution: ExecutionContext,
    pub block: BlockContext,
    pub block_hashes: BlockHashes,
}

impl SnapshotAnchor {
    pub const fn block_number(&self) -> u64 {
        self.block.number
    }

    pub const fn block_hash(&self) -> B256 {
        self.block.hash
    }

    pub const fn state_root(&self) -> B256 {
        self.block.state_root
    }
}

/// Persistent account fields and bytecode at one anchor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotAccount {
    pub account: AccountSnapshot,
    pub code: Bytes,
}

impl SnapshotAccount {
    pub const fn address(&self) -> Address {
        self.account.address
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageRangeEnd {
    Exhausted,
    HashLimit,
    ByteLimit,
}

impl StorageRangeEnd {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exhausted => "exhausted",
            Self::HashLimit => "hash limit",
            Self::ByteLimit => "byte limit",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoragePage {
    pub entries: Vec<(B256, U256)>,
    pub end: StorageRangeEnd,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SnapshotStats {
    pub pages: usize,
    pub entries: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageProgress {
    pub page: usize,
    pub entries: usize,
    pub total_entries: u64,
    pub end: StorageRangeEnd,
}

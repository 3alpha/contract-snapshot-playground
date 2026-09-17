use crate::{SnapshotAccount, SnapshotAnchor, StoragePage};
use alloy_primitives::{Address, B256};
use std::error::Error;

/// Root-pinned account and storage reads shared by snapshot producers.
///
/// Implementations pin every call to the supplied [`SnapshotAnchor`] so one batch of accounts
/// always describes the same canonical block.
pub trait ContractStateSource {
    type Error: Error + Send + Sync + 'static;

    fn anchor(&self) -> Result<SnapshotAnchor, Self::Error>;

    fn anchor_at(&self, block_number: u64) -> Result<SnapshotAnchor, Self::Error>;

    fn account(
        &self,
        anchor: &SnapshotAnchor,
        address: Address,
    ) -> Result<SnapshotAccount, Self::Error>;

    fn storage_range(
        &self,
        anchor: &SnapshotAnchor,
        address: Address,
        start: B256,
        page_bytes: usize,
    ) -> Result<StoragePage, Self::Error>;
}

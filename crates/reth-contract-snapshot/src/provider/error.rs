use alloy_primitives::{Address, B256};
use reth_provider::ProviderError;
use snapshot_format::SnapshotFormatError;
use std::{error::Error, fmt, path::PathBuf};

#[derive(Debug)]
pub enum RethSourceError {
    InvalidDatadir(PathBuf),
    OpenDatadir {
        path: PathBuf,
        reason: String,
    },
    Provider {
        operation: &'static str,
        source: ProviderError,
    },
    Format(SnapshotFormatError),
    AnchorHeaderUnavailable(u64),
    AnchorNotCanonical {
        number: u64,
        expected: B256,
        actual: B256,
    },
    AnchorStateRootChanged {
        number: u64,
        expected: B256,
        actual: B256,
    },
    StateRootNotRetained(B256),
    RequestedBlockAhead {
        requested: u64,
        latest: u64,
    },
    CanonicalHeaderMissing(u64),
    CanonicalHeadMismatch {
        number: u64,
        expected: B256,
        actual: B256,
    },
    MissingBlobParameters(u64),
    BlockHashWindowIncomplete {
        expected: usize,
        actual: usize,
    },
    AccountNotFound {
        address: Address,
        block: u64,
    },
    AccountDisappeared(Address),
    AccountMismatch {
        address: Address,
    },
    StorageRootMismatch {
        address: Address,
        range_root: B256,
        state_root: B256,
    },
    BytecodeMissing(B256),
    AccountAbsentFromTrie {
        address: Address,
        state_root: B256,
    },
}

impl fmt::Display for RethSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDatadir(path) => {
                write!(formatter, "Reth datadir does not exist: {}", path.display())
            }
            Self::OpenDatadir { path, reason } => write!(
                formatter,
                "failed to open Reth datadir {} read-only: {reason}",
                path.display()
            ),
            Self::Provider { operation, .. } => {
                write!(formatter, "Reth provider failed while {operation}")
            }
            Self::Format(_) => {
                formatter.write_str("resolved Reth data cannot form a valid snapshot")
            }
            Self::AnchorHeaderUnavailable(number) => write!(
                formatter,
                "snapshot anchor header {number} is no longer available"
            ),
            Self::AnchorNotCanonical {
                number,
                expected,
                actual,
            } => write!(
                formatter,
                "snapshot anchor is no longer canonical at block {number}: expected {expected}, got {actual}"
            ),
            Self::AnchorStateRootChanged {
                number,
                expected,
                actual,
            } => write!(
                formatter,
                "snapshot anchor state root changed at block {number}: expected {expected}, got {actual}"
            ),
            Self::StateRootNotRetained(root) => write!(
                formatter,
                "state root {root} is no longer retained; refusing to switch states"
            ),
            Self::RequestedBlockAhead { requested, latest } => write!(
                formatter,
                "requested block {requested} is ahead of latest canonical block {latest}"
            ),
            Self::CanonicalHeaderMissing(number) => {
                write!(formatter, "canonical header for block {number} is missing")
            }
            Self::CanonicalHeadMismatch {
                number,
                expected,
                actual,
            } => write!(
                formatter,
                "canonical head mismatch at block {number}: expected {expected}, got {actual}"
            ),
            Self::MissingBlobParameters(number) => write!(
                formatter,
                "block {number} requires blob parameters but the chain spec returned none"
            ),
            Self::BlockHashWindowIncomplete { expected, actual } => write!(
                formatter,
                "canonical block-hash window returned {actual} entries; expected {expected}"
            ),
            Self::AccountNotFound { address, block } => write!(
                formatter,
                "account {address} does not exist at block {block}"
            ),
            Self::AccountDisappeared(address) => {
                write!(formatter, "account {address} disappeared from pinned state")
            }
            Self::AccountMismatch { address } => write!(
                formatter,
                "account {address} disagrees between pinned state providers"
            ),
            Self::StorageRootMismatch {
                address,
                range_root,
                state_root,
            } => write!(
                formatter,
                "storage root for {address} disagrees between pinned providers: range {range_root}, state {state_root}"
            ),
            Self::BytecodeMissing(hash) => {
                write!(formatter, "bytecode {hash} is missing from pinned state")
            }
            Self::AccountAbsentFromTrie {
                address,
                state_root,
            } => write!(
                formatter,
                "account {address} is absent from the trie at pinned state root {state_root}"
            ),
        }
    }
}

impl Error for RethSourceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Provider { source, .. } => Some(source),
            Self::Format(source) => Some(source),
            _ => None,
        }
    }
}

impl From<SnapshotFormatError> for RethSourceError {
    fn from(source: SnapshotFormatError) -> Self {
        Self::Format(source)
    }
}

pub(crate) trait ProviderResultExt<T> {
    fn with_operation(self, operation: &'static str) -> Result<T, RethSourceError>;
}

impl<T> ProviderResultExt<T> for Result<T, ProviderError> {
    fn with_operation(self, operation: &'static str) -> Result<T, RethSourceError> {
        self.map_err(|source| RethSourceError::Provider { operation, source })
    }
}

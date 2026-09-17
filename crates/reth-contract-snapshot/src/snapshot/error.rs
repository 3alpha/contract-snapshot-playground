use alloy_primitives::B256;
use snapshot_format::SnapshotFormatError;
use std::{error::Error, fmt, io};

#[derive(Debug)]
pub enum SnapshotError {
    InvalidPageBytes,
    BytecodeHashMismatch {
        expected: B256,
        actual: B256,
    },
    StorageRootMismatch {
        expected: B256,
        actual: B256,
    },
    StoragePage {
        page: usize,
        source: Box<dyn Error + Send + Sync>,
    },
    EmptyNonExhaustedPage,
    ZeroStorageValue {
        key: B256,
    },
    DuplicateStorageKey {
        key: B256,
    },
    StorageKeysOutOfOrder {
        previous: B256,
        current: B256,
    },
    StorageKeyBeforeStart {
        key: B256,
        start: B256,
    },
    EntryCountOverflow,
    EmptyHashLimitedPage,
    HashLimitBeforeMaximum {
        last: B256,
    },
    MaximumKeyWithoutExhaustion,
    EmptyByteLimitedPage,
    ContinuationOverflow {
        last: B256,
    },
    NoPaginationProgress {
        start: B256,
        next: B256,
    },
    Format(SnapshotFormatError),
    Io(io::Error),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPageBytes => {
                formatter.write_str("page byte limit must be greater than zero")
            }
            Self::BytecodeHashMismatch { expected, actual } => write!(
                formatter,
                "bytecode hash mismatch: expected {expected}, got {actual}"
            ),
            Self::StorageRootMismatch { expected, actual } => write!(
                formatter,
                "complete storage root mismatch: expected {expected}, got {actual}"
            ),
            Self::StoragePage { page, .. } => write!(formatter, "storage page {page} failed"),
            Self::EmptyNonExhaustedPage => {
                formatter.write_str("storage pagination made no progress: empty non-exhausted page")
            }
            Self::ZeroStorageValue { key } => {
                write!(formatter, "provider returned zero-valued storage key {key}")
            }
            Self::DuplicateStorageKey { key } => write!(formatter, "duplicate storage key {key}"),
            Self::StorageKeysOutOfOrder { previous, current } => write!(
                formatter,
                "storage keys are not strictly increasing: {current} followed {previous}"
            ),
            Self::StorageKeyBeforeStart { key, start } => write!(
                formatter,
                "provider returned storage key {key} before start {start}"
            ),
            Self::EntryCountOverflow => formatter.write_str("storage entry count overflow"),
            Self::EmptyHashLimitedPage => {
                formatter.write_str("hash-limited storage page contained no entries")
            }
            Self::HashLimitBeforeMaximum { last } => write!(
                formatter,
                "provider reported HashLimit before the requested maximum key: {last}"
            ),
            Self::MaximumKeyWithoutExhaustion => formatter.write_str(
                "storage ended at the maximum possible key without explicit provider exhaustion",
            ),
            Self::EmptyByteLimitedPage => {
                formatter.write_str("byte-limited storage page contained no entries")
            }
            Self::ContinuationOverflow { last } => write!(
                formatter,
                "storage continuation overflowed after {last}; explicit exhaustion was not reached"
            ),
            Self::NoPaginationProgress { start, next } => write!(
                formatter,
                "storage pagination made no forward progress: start {start}, next {next}"
            ),
            Self::Format(_) => {
                formatter.write_str("snapshot v1 serialization or validation failed")
            }
            Self::Io(_) => formatter.write_str("snapshot output I/O failed"),
        }
    }
}

impl Error for SnapshotError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::StoragePage { source, .. } => Some(source.as_ref()),
            Self::Format(source) => Some(source),
            Self::Io(source) => Some(source),
            _ => None,
        }
    }
}

impl From<SnapshotFormatError> for SnapshotError {
    fn from(source: SnapshotFormatError) -> Self {
        Self::Format(source)
    }
}

impl From<serde_json::Error> for SnapshotError {
    fn from(source: serde_json::Error) -> Self {
        Self::Format(SnapshotFormatError::from(source))
    }
}

impl From<io::Error> for SnapshotError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

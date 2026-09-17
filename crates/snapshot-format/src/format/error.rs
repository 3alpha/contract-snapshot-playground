use alloy_primitives::{Address, B256};
use std::{error::Error, fmt};

#[derive(Debug)]
pub enum SnapshotFormatError {
    Json(serde_json::Error),
    UnsupportedVersion {
        actual: u64,
    },
    EmptyAccountList,
    DuplicateAccountAddress(Address),
    NonCanonicalQuantity {
        field: &'static str,
        value: String,
    },
    QuantityOverflow {
        field: &'static str,
        value: String,
    },
    InvalidFixedHex {
        field: &'static str,
        value: String,
        bytes: usize,
    },
    InvalidBytesHex {
        field: &'static str,
        value: String,
    },
    UnknownHardfork(String),
    DuplicateStorageKey(B256),
    StorageKeysOutOfOrder {
        previous: B256,
        current: B256,
    },
    ZeroStorageValue(B256),
    BytecodeHashMismatch {
        expected: B256,
        actual: B256,
    },
    StorageRootMismatch {
        expected: B256,
        actual: B256,
    },
    DuplicateBlockNumber(u64),
    BlockNumbersOutOfOrder {
        previous: u64,
        current: u64,
    },
    BlockHashWindowLength {
        expected: usize,
        actual: usize,
    },
    BlockHashNumber {
        expected: u64,
        actual: u64,
    },
    ParentHashMismatch {
        expected: B256,
        actual: B256,
    },
    MissingForkField {
        hardfork: &'static str,
        field: &'static str,
    },
    UnexpectedForkField {
        hardfork: &'static str,
        field: &'static str,
    },
    InvalidContextField {
        field: &'static str,
        reason: &'static str,
    },
}

impl fmt::Display for SnapshotFormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(_) => formatter.write_str("snapshot JSON is invalid"),
            Self::UnsupportedVersion { actual } => {
                write!(
                    formatter,
                    "unsupported snapshot version {actual}; expected version 1"
                )
            }
            Self::EmptyAccountList => formatter.write_str("snapshot contains no accounts"),
            Self::DuplicateAccountAddress(address) => {
                write!(formatter, "duplicate account address {address}")
            }
            Self::NonCanonicalQuantity { field, value } => {
                write!(
                    formatter,
                    "{field} is not a canonical Ethereum quantity: {value}"
                )
            }
            Self::QuantityOverflow { field, value } => {
                write!(formatter, "{field} is outside its supported range: {value}")
            }
            Self::InvalidFixedHex {
                field,
                value,
                bytes,
            } => write!(
                formatter,
                "{field} must be canonical 0x-prefixed lowercase hex containing {bytes} bytes: {value}"
            ),
            Self::InvalidBytesHex { field, value } => write!(
                formatter,
                "{field} must be canonical 0x-prefixed lowercase byte hex: {value}"
            ),
            Self::UnknownHardfork(value) => write!(formatter, "unknown EVM hardfork: {value}"),
            Self::DuplicateStorageKey(key) => write!(formatter, "duplicate storage key {key}"),
            Self::StorageKeysOutOfOrder { previous, current } => write!(
                formatter,
                "storage keys are not strictly increasing: {current} followed {previous}"
            ),
            Self::ZeroStorageValue(key) => write!(formatter, "storage key {key} has a zero value"),
            Self::BytecodeHashMismatch { expected, actual } => write!(
                formatter,
                "bytecode hash mismatch: expected {expected}, got {actual}"
            ),
            Self::StorageRootMismatch { expected, actual } => write!(
                formatter,
                "storage root mismatch: expected {expected}, got {actual}"
            ),
            Self::DuplicateBlockNumber(number) => {
                write!(formatter, "duplicate block hash entry for block {number}")
            }
            Self::BlockNumbersOutOfOrder { previous, current } => write!(
                formatter,
                "block hash numbers are not strictly increasing: {current} followed {previous}"
            ),
            Self::BlockHashWindowLength { expected, actual } => write!(
                formatter,
                "block hash window has {actual} entries; expected {expected}"
            ),
            Self::BlockHashNumber { expected, actual } => write!(
                formatter,
                "block hash window contains block {actual}; expected block {expected}"
            ),
            Self::ParentHashMismatch { expected, actual } => write!(
                formatter,
                "last historical block hash is {actual}; header parent hash is {expected}"
            ),
            Self::MissingForkField { hardfork, field } => {
                write!(formatter, "{field} is required for the {hardfork} hardfork")
            }
            Self::UnexpectedForkField { hardfork, field } => {
                write!(
                    formatter,
                    "{field} is not valid for the {hardfork} hardfork"
                )
            }
            Self::InvalidContextField { field, reason } => {
                write!(formatter, "{field} is invalid: {reason}")
            }
        }
    }
}

impl Error for SnapshotFormatError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for SnapshotFormatError {
    fn from(source: serde_json::Error) -> Self {
        Self::Json(source)
    }
}

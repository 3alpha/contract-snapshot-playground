use alloy_primitives::{Address, B256};
use snapshot_format::SnapshotFormatError;
use std::{error::Error, fmt};

#[derive(Debug)]
pub enum ExecutionError {
    Snapshot(SnapshotFormatError),
    SnapshotContextMismatch,
    DuplicateAccount(Address),
    MissingAccount(Address),
    MissingBlockHash(u64),
    MissingBytecode(B256),
    MissingAbi(Address),
    InvalidBytecode { address: Address, reason: String },
    InvalidAddress { field: &'static str, value: String },
    InvalidQuantity { field: &'static str, value: String },
    InvalidCalldata(String),
    InvalidGasLimit,
    CallGasLimitTooHigh { requested: u64, maximum: u64 },
    CallerSnapshotRequired(Address),
    ExecutionContextMissing,
    BlobFractionOverflow(u128),
    Transaction(String),
    Database(Box<ExecutionError>),
    Abi(crate::AbiError),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Snapshot(_) => formatter.write_str("snapshot validation failed"),
            Self::SnapshotContextMismatch => formatter.write_str(
                "snapshot execution context or anchor does not match the loaded workspace",
            ),
            Self::DuplicateAccount(address) => {
                write!(formatter, "account {address} is already loaded")
            }
            Self::MissingAccount(address) => write!(
                formatter,
                "execution requested unsnapshotted account {address}"
            ),
            Self::MissingBlockHash(number) => write!(
                formatter,
                "required BLOCKHASH value for block {number} is missing"
            ),
            Self::MissingBytecode(hash) => {
                write!(formatter, "snapshotted bytecode {hash} is unavailable")
            }
            Self::MissingAbi(address) => {
                write!(formatter, "no validated ABI is loaded for {address}")
            }
            Self::InvalidBytecode { address, reason } => {
                write!(formatter, "bytecode for {address} is invalid: {reason}")
            }
            Self::InvalidAddress { field, value } => {
                write!(formatter, "{field} is not a valid address: {value}")
            }
            Self::InvalidQuantity { field, value } => write!(
                formatter,
                "{field} is not a decimal or 0x-prefixed quantity: {value}"
            ),
            Self::InvalidCalldata(value) => write!(
                formatter,
                "calldata must be even-length 0x-prefixed hex: {value}"
            ),
            Self::InvalidGasLimit => formatter.write_str("gas limit must be greater than zero"),
            Self::CallGasLimitTooHigh { requested, maximum } => write!(
                formatter,
                "gas limit {requested} exceeds the browser safety cap of {maximum}"
            ),
            Self::CallerSnapshotRequired(address) => write!(
                formatter,
                "caller {address} must be snapshotted for a non-zero-value call"
            ),
            Self::ExecutionContextMissing => {
                formatter.write_str("load at least one snapshot before executing a call")
            }
            Self::BlobFractionOverflow(value) => write!(
                formatter,
                "blob base fee update fraction {value} does not fit REVM's u64 environment"
            ),
            Self::Transaction(reason) => {
                write!(formatter, "REVM rejected the call environment: {reason}")
            }
            Self::Database(_) => formatter.write_str("REVM database access failed"),
            Self::Abi(_) => formatter.write_str("ABI operation failed"),
        }
    }
}

impl Error for ExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Snapshot(source) => Some(source),
            Self::Database(source) => Some(source.as_ref()),
            Self::Abi(source) => Some(source),
            _ => None,
        }
    }
}

impl revm::database_interface::DBErrorMarker for ExecutionError {}

impl From<SnapshotFormatError> for ExecutionError {
    fn from(source: SnapshotFormatError) -> Self {
        Self::Snapshot(source)
    }
}

impl From<crate::AbiError> for ExecutionError {
    fn from(source: crate::AbiError) -> Self {
        Self::Abi(source)
    }
}

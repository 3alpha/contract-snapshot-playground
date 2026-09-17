use alloy_primitives::Address;
use reth_contract_snapshot::{SnapshotError, provider::RethSourceError};
use std::{error::Error, fmt, io, num::ParseIntError, path::PathBuf};

#[derive(Debug)]
pub(crate) enum AppError {
    InvalidPageBytes,
    DuplicateAddress(Address),
    Reth(RethSourceError),
    Snapshot(SnapshotError),
    Io(io::Error),
    OutputDirectoryMissing(PathBuf),
    CreateOutputStagingFile {
        directory: PathBuf,
        source: io::Error,
    },
    FinalizeOutput {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPageBytes => formatter.write_str("--page-bytes must be greater than zero"),
            Self::DuplicateAddress(address) => {
                write!(formatter, "address {address} was requested more than once")
            }
            Self::Reth(_) => formatter.write_str("failed to read Reth state"),
            Self::Snapshot(_) => formatter.write_str("failed to create snapshot"),
            Self::Io(_) => formatter.write_str("snapshot file or stdout I/O failed"),
            Self::OutputDirectoryMissing(path) => write!(
                formatter,
                "output directory does not exist: {}",
                path.display()
            ),
            Self::CreateOutputStagingFile { directory, .. } => write!(
                formatter,
                "failed to create a temporary output file in {}",
                directory.display()
            ),
            Self::FinalizeOutput { path, .. } => write!(
                formatter,
                "failed to finalize output file {}",
                path.display()
            ),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Reth(source) => Some(source),
            Self::Snapshot(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::CreateOutputStagingFile { source, .. } | Self::FinalizeOutput { source, .. } => {
                Some(source)
            }
            Self::InvalidPageBytes
            | Self::DuplicateAddress(_)
            | Self::OutputDirectoryMissing(_) => None,
        }
    }
}

impl From<RethSourceError> for AppError {
    fn from(source: RethSourceError) -> Self {
        Self::Reth(source)
    }
}

impl From<SnapshotError> for AppError {
    fn from(source: SnapshotError) -> Self {
        Self::Snapshot(source)
    }
}

impl From<io::Error> for AppError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

#[derive(Debug)]
pub(super) enum BlockNumberParseError {
    EmptyHex,
    InvalidHex(ParseIntError),
    InvalidDecimal(ParseIntError),
}

impl fmt::Display for BlockNumberParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyHex => formatter.write_str("hex block number must contain a digit"),
            Self::InvalidHex(source) => {
                write!(formatter, "invalid hexadecimal block number: {source}")
            }
            Self::InvalidDecimal(source) => {
                write!(formatter, "invalid decimal block number: {source}")
            }
        }
    }
}

impl Error for BlockNumberParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidHex(source) | Self::InvalidDecimal(source) => Some(source),
            Self::EmptyHex => None,
        }
    }
}

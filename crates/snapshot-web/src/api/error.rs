use crate::ExecutionError;
use std::{error::Error, fmt};

#[derive(Debug)]
pub(super) enum ApiError {
    RequestJson(serde_json::Error),
    Execution(ExecutionError),
}

impl ApiError {
    pub(super) const fn code(&self) -> &'static str {
        match self {
            Self::RequestJson(_) => "invalid-request",
            Self::Execution(ExecutionError::MissingAccount(_)) => "missing-account",
            Self::Execution(ExecutionError::MissingBlockHash(_)) => "missing-block-hash",
            Self::Execution(ExecutionError::DuplicateAccount(_)) => "duplicate-account",
            Self::Execution(ExecutionError::SnapshotContextMismatch) => "snapshot-context-mismatch",
            Self::Execution(ExecutionError::CallerSnapshotRequired(_)) => {
                "caller-snapshot-required"
            }
            Self::Execution(ExecutionError::Snapshot(_)) => "invalid-snapshot",
            Self::Execution(ExecutionError::MissingAbi(_)) => "abi-error",
            Self::Execution(ExecutionError::Abi(_)) => "abi-error",
            Self::Execution(_) => "execution-error",
        }
    }

    pub(super) fn causes(&self) -> Vec<String> {
        let mut causes = Vec::new();
        let mut source = self.source();
        while let Some(cause) = source {
            causes.push(cause.to_string());
            source = cause.source();
        }
        causes
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestJson(_) => formatter.write_str("call request JSON is invalid"),
            Self::Execution(_) => formatter.write_str("snapshot execution request failed"),
        }
    }
}

impl Error for ApiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RequestJson(source) => Some(source),
            Self::Execution(source) => Some(source),
        }
    }
}

impl From<ExecutionError> for ApiError {
    fn from(source: ExecutionError) -> Self {
        Self::Execution(source)
    }
}

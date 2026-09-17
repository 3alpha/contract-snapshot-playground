use std::{error::Error, fmt};

#[derive(Debug)]
pub enum AbiError {
    Json(serde_json::Error),
    FunctionNotFound(String),
    ArgumentCount {
        signature: String,
        expected: usize,
        actual: usize,
    },
    ResolveType {
        parameter: String,
        reason: String,
    },
    InvalidArgument {
        signature: String,
        index: usize,
        value: String,
        reason: String,
    },
    Encode {
        signature: String,
        reason: String,
    },
    DecodeOutput {
        signature: String,
        reason: String,
    },
}

impl fmt::Display for AbiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(_) => formatter.write_str("contract ABI is not valid JSON ABI"),
            Self::FunctionNotFound(signature) => {
                write!(formatter, "ABI function {signature} was not found")
            }
            Self::ArgumentCount {
                signature,
                expected,
                actual,
            } => write!(
                formatter,
                "ABI function {signature} expects {expected} arguments, got {actual}"
            ),
            Self::ResolveType { parameter, reason } => write!(
                formatter,
                "cannot resolve ABI type for {parameter}: {reason}"
            ),
            Self::InvalidArgument {
                signature,
                index,
                value,
                reason,
            } => write!(
                formatter,
                "argument {index} for {signature} is invalid ({value}): {reason}"
            ),
            Self::Encode { signature, reason } => {
                write!(formatter, "failed to encode {signature}: {reason}")
            }
            Self::DecodeOutput { signature, reason } => write!(
                formatter,
                "failed to decode output from {signature}: {reason}"
            ),
        }
    }
}

impl Error for AbiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            _ => None,
        }
    }
}

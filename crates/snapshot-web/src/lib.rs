//! WASM-facing, browser-local EVM execution over validated snapshot v1 batch files.

mod abi;
mod api;
mod executor;

pub use abi::{AbiDescription, AbiError, AbiFunction, AbiParameter, ContractAbi};
pub use api::SnapshotExecutor;
pub use executor::{
    CallInput, CallRequest, CallResult, ExecutionError, LoadedAccountSummary, SnapshotWorkspace,
};

#[cfg(test)]
mod tests;

mod error;
mod writer;

pub use error::SnapshotError;
pub use writer::{key_successor, validate_bytecode, write_accounts};

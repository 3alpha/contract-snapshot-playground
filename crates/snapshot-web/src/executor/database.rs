use super::ExecutionError;
use alloy_primitives::{Address, B256, U256, keccak256};
use revm::{Database, bytecode::Bytecode, state::AccountInfo};
use snapshot_format::{AccountSnapshot, BlockContext, BlockHashes, StorageEntries};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) struct LoadedAccount {
    pub fields: AccountSnapshot,
    pub code: Bytecode,
    pub code_size: usize,
    pub storage: StorageEntries,
}

pub(super) struct SnapshotDatabase<'a> {
    pub accounts: &'a BTreeMap<Address, LoadedAccount>,
    pub code: &'a BTreeMap<B256, Bytecode>,
    pub block: &'a BlockContext,
    pub block_hashes: &'a BlockHashes,
    pub empty_caller: Option<Address>,
}

impl Database for SnapshotDatabase<'_> {
    type Error = ExecutionError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(account) = self.accounts.get(&address) {
            return Ok(Some(AccountInfo::new(
                account.fields.balance,
                account.fields.nonce,
                account.fields.code_hash,
                account.code.clone(),
            )));
        }
        if self.empty_caller == Some(address) {
            return Ok(None);
        }
        Err(ExecutionError::MissingAccount(address))
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.code
            .get(&code_hash)
            .cloned()
            .ok_or(ExecutionError::MissingBytecode(code_hash))
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let account = self
            .accounts
            .get(&address)
            .ok_or(ExecutionError::MissingAccount(address))?;
        let hashed_slot = keccak256(index.to_be_bytes::<32>());
        Ok(account.storage.get(&hashed_slot).unwrap_or(U256::ZERO))
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        if number >= self.block.number || self.block.number - number > 256 {
            return Ok(B256::ZERO);
        }
        self.block_hashes
            .get(number)
            .ok_or(ExecutionError::MissingBlockHash(number))
    }
}

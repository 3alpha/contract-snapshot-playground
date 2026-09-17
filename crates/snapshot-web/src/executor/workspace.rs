use super::{
    CallInput, CallRequest, CallResult, ExecutionError, LoadedAccountSummary,
    database::{LoadedAccount, SnapshotDatabase},
};
use crate::{AbiDescription, ContractAbi};
use alloy_primitives::{Address, Bytes, U256, hex};
use revm::{
    Context, ExecuteEvm, MainBuilder, MainContext,
    bytecode::Bytecode,
    context::{BlockEnv, TxEnv},
    context_interface::result::{EVMError, ExecutionResult},
    primitives::hardfork::SpecId,
};
use snapshot_format::{BlockContext, BlockHashes, EvmHardfork, ExecutionContext, Snapshot};
use std::{collections::BTreeMap, str::FromStr};

const MAX_CALL_GAS: u64 = 100_000_000;
const MAX_MEMORY_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceContext {
    execution: ExecutionContext,
    block: BlockContext,
    block_hashes: BlockHashes,
}

#[derive(Debug, Default)]
pub struct SnapshotWorkspace {
    context: Option<WorkspaceContext>,
    accounts: BTreeMap<Address, LoadedAccount>,
    code: BTreeMap<alloy_primitives::B256, Bytecode>,
    abis: BTreeMap<Address, ContractAbi>,
}

impl SnapshotWorkspace {
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads a complete v1 batch envelope, merging every account into the workspace.
    ///
    /// The batch fails entirely if its shared context differs from the context of already-loaded
    /// accounts, or if any address is already present.
    pub fn add_batch(&mut self, bytes: &[u8]) -> Result<Vec<LoadedAccountSummary>, ExecutionError> {
        let snapshot = Snapshot::from_slice(bytes)?;
        let candidate_context = WorkspaceContext {
            execution: snapshot.execution.clone(),
            block: snapshot.block.clone(),
            block_hashes: snapshot.block_hashes.clone(),
        };
        if let Some(current) = &self.context
            && current != &candidate_context
        {
            return Err(ExecutionError::SnapshotContextMismatch);
        }
        for record in &snapshot.accounts {
            if self.accounts.contains_key(&record.account.address) {
                return Err(ExecutionError::DuplicateAccount(record.account.address));
            }
        }

        let mut prepared = Vec::with_capacity(snapshot.accounts.len());
        for record in snapshot.accounts {
            let code_size = record.code.len();
            let bytecode = Bytecode::new_raw_checked(record.code.clone()).map_err(|source| {
                ExecutionError::InvalidBytecode {
                    address: record.account.address,
                    reason: source.to_string(),
                }
            })?;
            let summary = LoadedAccountSummary {
                address: record.account.address.to_string().to_lowercase(),
                nonce: format!("{:#x}", record.account.nonce),
                balance: format!("{:#x}", record.account.balance),
                code_size,
                storage_entries: record.storage.as_slice().len(),
                block_number: format!("{:#x}", snapshot.block.number),
                block_hash: snapshot.block.hash.to_string(),
            };
            let account = LoadedAccount {
                fields: record.account,
                code: bytecode,
                code_size,
                storage: record.storage,
            };
            prepared.push((summary, account));
        }

        let mut summaries = Vec::with_capacity(prepared.len());
        for (summary, account) in prepared {
            self.code
                .insert(account.fields.code_hash, account.code.clone());
            self.accounts.insert(account.fields.address, account);
            summaries.push(summary);
        }
        if self.context.is_none() {
            self.context = Some(candidate_context);
        }
        Ok(summaries)
    }

    pub fn remove_snapshot(&mut self, address: &str) -> Result<(), ExecutionError> {
        let address = parse_address("address", address)?;
        let removed = self
            .accounts
            .remove(&address)
            .ok_or(ExecutionError::MissingAccount(address))?;
        let code_is_still_used = self
            .accounts
            .values()
            .any(|account| account.fields.code_hash == removed.fields.code_hash);
        if !code_is_still_used {
            self.code.remove(&removed.fields.code_hash);
        }
        if self.accounts.is_empty() {
            self.context = None;
        }
        Ok(())
    }

    pub fn accounts(&self) -> Vec<LoadedAccountSummary> {
        let Some(context) = &self.context else {
            return Vec::new();
        };
        self.accounts
            .values()
            .map(|account| LoadedAccountSummary {
                address: account.fields.address.to_string().to_lowercase(),
                nonce: format!("{:#x}", account.fields.nonce),
                balance: format!("{:#x}", account.fields.balance),
                code_size: account.code_size,
                storage_entries: account.storage.as_slice().len(),
                block_number: format!("{:#x}", context.block.number),
                block_hash: context.block.hash.to_string(),
            })
            .collect()
    }

    pub fn set_abi(&mut self, address: &str, json: &str) -> Result<AbiDescription, ExecutionError> {
        let address = parse_address("ABI address", address)?;
        let abi = ContractAbi::parse(json)?;
        let description = abi.describe();
        self.abis.insert(address, abi);
        Ok(description)
    }

    pub fn remove_abi(&mut self, address: &str) -> Result<(), ExecutionError> {
        let address = parse_address("ABI address", address)?;
        self.abis.remove(&address);
        Ok(())
    }

    pub fn execute(&self, request: CallRequest) -> Result<CallResult, ExecutionError> {
        let context = self
            .context
            .as_ref()
            .ok_or(ExecutionError::ExecutionContextMissing)?;
        let target = parse_address("target", &request.target)?;
        let caller = parse_address("caller", &request.caller)?;
        let value = parse_quantity("value", &request.value)?;
        if request.gas_limit == 0 {
            return Err(ExecutionError::InvalidGasLimit);
        }
        if request.gas_limit > MAX_CALL_GAS {
            return Err(ExecutionError::CallGasLimitTooHigh {
                requested: request.gas_limit,
                maximum: MAX_CALL_GAS,
            });
        }
        if !self.accounts.contains_key(&target) {
            return Err(ExecutionError::MissingAccount(target));
        }
        let caller_is_loaded = self.accounts.contains_key(&caller);
        if !value.is_zero() && !caller_is_loaded {
            return Err(ExecutionError::CallerSnapshotRequired(caller));
        }

        let (calldata, abi_signature) = match &request.input {
            CallInput::Raw { calldata } => (parse_bytes(calldata)?, None),
            CallInput::Abi {
                signature,
                arguments,
            } => {
                let abi = self
                    .abis
                    .get(&target)
                    .ok_or(ExecutionError::MissingAbi(target))?;
                (
                    Bytes::from(abi.encode_call(signature, arguments)?),
                    Some(signature.as_str()),
                )
            }
        };

        let empty_caller = (!caller_is_loaded).then_some(caller);
        let database = SnapshotDatabase {
            accounts: &self.accounts,
            code: &self.code,
            block: &context.block,
            block_hashes: &context.block_hashes,
            empty_caller,
        };
        let spec_id = spec_id(context.execution.hardfork);
        let fraction = context
            .execution
            .blob_base_fee_update_fraction
            .map(|value| {
                u64::try_from(value).map_err(|_| ExecutionError::BlobFractionOverflow(value))
            })
            .transpose()?;
        let mut block = BlockEnv {
            number: U256::from(context.block.number),
            beneficiary: context.block.beneficiary,
            timestamp: U256::from(context.block.timestamp),
            gas_limit: context.block.gas_limit,
            basefee: context.block.base_fee_per_gas.unwrap_or(0),
            difficulty: context.block.difficulty,
            prevrandao: context.block.prev_randao,
            blob_excess_gas_and_price: None,
            slot_num: context.block.slot_number.unwrap_or(0),
        };
        if let (Some(excess), Some(fraction)) = (context.block.excess_blob_gas, fraction) {
            block.set_blob_excess_gas_and_price(excess, fraction);
        }

        let evm_context = Context::mainnet()
            .modify_cfg_chained(|cfg| {
                cfg.set_spec_and_mainnet_gas_params(spec_id);
                cfg.chain_id = context.execution.chain_id;
                cfg.disable_nonce_check = true;
                cfg.disable_balance_check = true;
                cfg.disable_block_gas_limit = true;
                cfg.disable_eip3607 = true;
                cfg.disable_base_fee = true;
                cfg.disable_fee_charge = true;
                cfg.memory_limit = MAX_MEMORY_BYTES;
                cfg.blob_base_fee_update_fraction = fraction;
            })
            .modify_block_chained(|environment| *environment = block)
            .with_db(database);
        let mut evm = evm_context.build_mainnet();
        let tx = TxEnv::builder()
            .caller(caller)
            .gas_limit(request.gas_limit)
            .gas_price(0)
            .to(target)
            .value(value)
            .data(calldata.clone())
            .nonce(0)
            .chain_id(Some(context.execution.chain_id))
            .build()
            .map_err(|source| ExecutionError::Transaction(source.to_string()))?;
        let result = evm.transact(tx).map_err(map_evm_error)?.result;
        Ok(format_result(
            result,
            &calldata,
            self.abis.get(&target),
            abi_signature,
        ))
    }
}

fn format_result(
    result: ExecutionResult,
    calldata: &Bytes,
    abi: Option<&ContractAbi>,
    signature: Option<&str>,
) -> CallResult {
    let calldata = format!("0x{}", hex::encode(calldata));
    match result {
        ExecutionResult::Success { gas, output, .. } => {
            let output = output.into_data();
            let decoding = abi
                .zip(signature)
                .map(|(abi, signature)| abi.decode_output(signature, &output));
            let (decoded, reason) = match decoding {
                Some(Ok(values)) => (Some(values), None),
                Some(Err(error)) => (None, Some(error.to_string())),
                None => (None, None),
            };
            CallResult {
                status: "success".to_owned(),
                gas_used: gas.tx_gas_used(),
                calldata,
                output: format!("0x{}", hex::encode(output)),
                decoded,
                reason,
            }
        }
        ExecutionResult::Revert { gas, output, .. } => {
            let reason = abi
                .and_then(|abi| abi.decode_revert(&output))
                .or_else(|| ContractAbi::decode_standard_revert(&output));
            CallResult {
                status: "revert".to_owned(),
                gas_used: gas.tx_gas_used(),
                calldata,
                output: format!("0x{}", hex::encode(output)),
                decoded: None,
                reason,
            }
        }
        ExecutionResult::Halt { reason, gas, .. } => CallResult {
            status: "halt".to_owned(),
            gas_used: gas.tx_gas_used(),
            calldata,
            output: "0x".to_owned(),
            decoded: None,
            reason: Some(reason.to_string()),
        },
    }
}

fn map_evm_error(error: EVMError<ExecutionError>) -> ExecutionError {
    match error {
        EVMError::Database(source) => ExecutionError::Database(Box::new(source)),
        other => ExecutionError::Transaction(other.to_string()),
    }
}

fn parse_address(field: &'static str, value: &str) -> Result<Address, ExecutionError> {
    Address::from_str(value).map_err(|_| ExecutionError::InvalidAddress {
        field,
        value: value.to_owned(),
    })
}

fn parse_quantity(field: &'static str, value: &str) -> Result<U256, ExecutionError> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        if hex.is_empty() {
            None
        } else {
            U256::from_str_radix(hex, 16).ok()
        }
    } else {
        U256::from_str_radix(value, 10).ok()
    };
    parsed.ok_or_else(|| ExecutionError::InvalidQuantity {
        field,
        value: value.to_owned(),
    })
}

fn parse_bytes(value: &str) -> Result<Bytes, ExecutionError> {
    let Some(hex) = value.strip_prefix("0x") else {
        return Err(ExecutionError::InvalidCalldata(value.to_owned()));
    };
    if hex.len() % 2 != 0 {
        return Err(ExecutionError::InvalidCalldata(value.to_owned()));
    }
    alloy_primitives::hex::decode(hex)
        .map(Bytes::from)
        .map_err(|_| ExecutionError::InvalidCalldata(value.to_owned()))
}

const fn spec_id(hardfork: EvmHardfork) -> SpecId {
    match hardfork {
        EvmHardfork::Frontier => SpecId::FRONTIER,
        EvmHardfork::Homestead => SpecId::HOMESTEAD,
        EvmHardfork::Tangerine => SpecId::TANGERINE,
        EvmHardfork::SpuriousDragon => SpecId::SPURIOUS_DRAGON,
        EvmHardfork::Byzantium => SpecId::BYZANTIUM,
        EvmHardfork::Constantinople | EvmHardfork::Petersburg => SpecId::PETERSBURG,
        EvmHardfork::Istanbul | EvmHardfork::MuirGlacier => SpecId::ISTANBUL,
        EvmHardfork::Berlin => SpecId::BERLIN,
        EvmHardfork::London | EvmHardfork::ArrowGlacier | EvmHardfork::GrayGlacier => {
            SpecId::LONDON
        }
        EvmHardfork::Paris => SpecId::MERGE,
        EvmHardfork::Shanghai => SpecId::SHANGHAI,
        EvmHardfork::Cancun => SpecId::CANCUN,
        EvmHardfork::Prague => SpecId::PRAGUE,
        EvmHardfork::Osaka => SpecId::OSAKA,
        EvmHardfork::Amsterdam => SpecId::AMSTERDAM,
    }
}

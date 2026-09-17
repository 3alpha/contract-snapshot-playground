use crate::{CallInput, CallRequest, ContractAbi, ExecutionError, SnapshotWorkspace};
use alloy_primitives::{Address, B256, Bytes, U256, keccak256};
use snapshot_format::{
    AccountRecord, AccountSnapshot, BlockContext, BlockHashes, EvmHardfork, ExecutionContext,
    Snapshot, StorageEntries, VERSION,
};

fn execution() -> ExecutionContext {
    ExecutionContext {
        chain_id: 1,
        hardfork: EvmHardfork::Prague,
        blob_base_fee_update_fraction: Some(3_338_477),
    }
}

fn block() -> BlockContext {
    BlockContext {
        number: 1,
        hash: B256::with_last_byte(2),
        parent_hash: B256::with_last_byte(1),
        state_root: B256::with_last_byte(3),
        timestamp: 1_700_000_000,
        beneficiary: Address::with_last_byte(9),
        gas_limit: 30_000_000,
        base_fee_per_gas: Some(1),
        difficulty: U256::ZERO,
        prev_randao: Some(B256::with_last_byte(4)),
        excess_blob_gas: Some(0),
        slot_number: None,
    }
}

fn block_hashes() -> BlockHashes {
    BlockHashes::new(vec![(0, B256::with_last_byte(1))]).unwrap()
}

fn record(address: Address, code: &[u8], storage: Vec<(B256, U256)>) -> AccountRecord {
    record_with_balance(address, code, storage, U256::ZERO)
}

fn record_with_balance(
    address: Address,
    code: &[u8],
    storage: Vec<(B256, U256)>,
    balance: U256,
) -> AccountRecord {
    let storage = StorageEntries::new(storage).unwrap();
    let code = Bytes::copy_from_slice(code);
    AccountRecord {
        account: AccountSnapshot {
            address,
            nonce: 1,
            balance,
            code_hash: keccak256(&code),
            storage_root: storage.root().unwrap(),
        },
        code,
        storage,
    }
}

fn batch(records: Vec<AccountRecord>) -> Vec<u8> {
    Snapshot {
        version: VERSION,
        execution: execution(),
        block: block(),
        block_hashes: block_hashes(),
        accounts: records,
    }
    .to_vec()
    .unwrap()
}

fn snapshot(address: Address, code: &[u8], storage: Vec<(B256, U256)>) -> Vec<u8> {
    batch(vec![record(address, code, storage)])
}

fn call(target: Address, calldata: &str) -> CallRequest {
    CallRequest {
        target: target.to_string(),
        caller: Address::with_last_byte(0xfe).to_string(),
        value: "0".to_owned(),
        gas_limit: 1_000_000,
        input: CallInput::Raw {
            calldata: calldata.to_owned(),
        },
    }
}

fn output_word(output: &str) -> U256 {
    U256::from_str_radix(output.strip_prefix("0x").unwrap(), 16).unwrap()
}

#[test]
fn hashes_raw_slots_before_sorted_lookup() {
    let target = Address::with_last_byte(0x81);
    let code = alloy_primitives::hex::decode("60005460005260206000f3").unwrap();
    let key = keccak256(B256::ZERO);
    let mut workspace = SnapshotWorkspace::new();
    workspace
        .add_batch(&snapshot(target, &code, vec![(key, U256::from(7))]))
        .unwrap();
    let result = workspace.execute(call(target, "0x")).unwrap();
    assert_eq!(result.status, "success");
    assert_eq!(output_word(&result.output), U256::from(7));
}

#[test]
fn missing_accounts_and_value_callers_are_typed_errors() {
    let target = Address::with_last_byte(0x81);
    let unknown = Address::repeat_byte(0x22);
    let mut code = vec![0x73];
    code.extend_from_slice(unknown.as_slice());
    code.extend_from_slice(&[0x3b, 0x00]);
    let mut workspace = SnapshotWorkspace::new();
    workspace
        .add_batch(&snapshot(target, &code, vec![]))
        .unwrap();
    assert!(matches!(
        workspace.execute(call(unknown, "0x")),
        Err(ExecutionError::MissingAccount(address)) if address == unknown
    ));
    assert!(matches!(
        workspace.execute(call(target, "0x")),
        Err(ExecutionError::Database(_))
    ));

    let value_target = Address::with_last_byte(0x89);
    workspace
        .add_batch(&snapshot(value_target, &[0x00], vec![]))
        .unwrap();
    let mut value_call = call(value_target, "0x");
    value_call.value = "1".to_owned();
    assert!(matches!(
        workspace.execute(value_call.clone()),
        Err(ExecutionError::CallerSnapshotRequired(_))
    ));

    let caller: Address = value_call.caller.parse().unwrap();
    workspace
        .add_batch(&batch(vec![record_with_balance(
            caller,
            &[],
            vec![],
            U256::ONE,
        )]))
        .unwrap();
    assert_eq!(workspace.execute(value_call).unwrap().status, "success");
}

#[test]
fn merge_requires_an_identical_anchor_and_execution_context() {
    let first = Address::with_last_byte(0x81);
    let second = Address::with_last_byte(0x82);
    let mut workspace = SnapshotWorkspace::new();
    workspace
        .add_batch(&snapshot(first, &[0x00], vec![]))
        .unwrap();
    workspace
        .add_batch(&snapshot(second, &[0x00], vec![]))
        .unwrap();
    assert_eq!(workspace.accounts().len(), 2);

    let mut mismatched: Snapshot =
        serde_json::from_slice(&snapshot(Address::with_last_byte(0x83), &[0x00], vec![])).unwrap();
    mismatched.block.hash = B256::with_last_byte(0xff);
    let bytes = serde_json::to_vec(&mismatched).unwrap();
    assert!(matches!(
        workspace.add_batch(&bytes),
        Err(ExecutionError::SnapshotContextMismatch)
    ));
}

#[test]
fn add_batch_loads_multiple_accounts_and_returns_summaries() {
    let first = Address::with_last_byte(0x81);
    let second = Address::with_last_byte(0x82);
    let bytes = batch(vec![
        record(first, &[0x00], vec![]),
        record(second, &[0x60, 0x00], vec![]),
    ]);
    let mut workspace = SnapshotWorkspace::new();
    let summaries = workspace.add_batch(&bytes).unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(workspace.accounts().len(), 2);

    // A second batch with a fresh address merges into the same workspace context.
    let third = Address::with_last_byte(0x83);
    workspace
        .add_batch(&snapshot(third, &[0x00], vec![]))
        .unwrap();
    assert_eq!(workspace.accounts().len(), 3);

    // But a batch that repeats an already-loaded address is rejected wholesale.
    let fourth = Address::with_last_byte(0x84);
    let bytes = batch(vec![
        record(first, &[0x00], vec![]),
        record(fourth, &[0x00], vec![]),
    ]);
    assert!(matches!(
        workspace.add_batch(&bytes),
        Err(ExecutionError::DuplicateAccount(address)) if address == first
    ));
    assert!(
        !workspace
            .accounts()
            .iter()
            .any(|account| account.address == fourth.to_string().to_lowercase())
    );
}

#[test]
fn add_batch_does_not_mutate_workspace_when_bytecode_is_invalid() {
    let valid = record(Address::with_last_byte(0x81), &[0x00], vec![]);
    let malformed_delegation = record(Address::with_last_byte(0x82), &[0xef, 0x01], vec![]);
    let bytes = batch(vec![valid, malformed_delegation]);
    let mut workspace = SnapshotWorkspace::new();

    assert!(matches!(
        workspace.add_batch(&bytes),
        Err(ExecutionError::InvalidBytecode { .. })
    ));
    assert!(workspace.accounts().is_empty());
}

#[test]
fn block_environment_is_captured_and_calls_do_not_commit() {
    let number_target = Address::with_last_byte(0x81);
    let number_code = alloy_primitives::hex::decode("4360005260206000f3").unwrap();
    let mut workspace = SnapshotWorkspace::new();
    workspace
        .add_batch(&snapshot(number_target, &number_code, vec![]))
        .unwrap();
    let number = workspace.execute(call(number_target, "0x")).unwrap();
    assert_eq!(output_word(&number.output), U256::ONE);

    let hash_target = Address::with_last_byte(0x83);
    let hash_code = alloy_primitives::hex::decode("60004060005260206000f3").unwrap();
    workspace
        .add_batch(&snapshot(hash_target, &hash_code, vec![]))
        .unwrap();
    let hash = workspace.execute(call(hash_target, "0x")).unwrap();
    assert_eq!(hash.output, B256::with_last_byte(1).to_string());

    let storage_target = Address::with_last_byte(0x82);
    let update_code =
        alloy_primitives::hex::decode("6000546001018060005560005260206000f3").unwrap();
    let key = keccak256(B256::ZERO);
    workspace
        .add_batch(&snapshot(
            storage_target,
            &update_code,
            vec![(key, U256::from(7))],
        ))
        .unwrap();
    let first = workspace.execute(call(storage_target, "0x")).unwrap();
    let second = workspace.execute(call(storage_target, "0x")).unwrap();
    assert_eq!(output_word(&first.output), U256::from(8));
    assert_eq!(first.output, second.output);
}

#[test]
fn abi_overloads_encode_and_results_and_reverts_decode() {
    let abi = ContractAbi::parse(
        r#"[
          {"type":"function","name":"read","stateMutability":"view","inputs":[{"name":"who","type":"address"}],"outputs":[{"name":"value","type":"uint256"}]},
          {"type":"function","name":"read","stateMutability":"view","inputs":[{"name":"slot","type":"uint256"}],"outputs":[{"name":"value","type":"uint256"}]},
          {"type":"error","name":"Unauthorized","inputs":[{"name":"caller","type":"address"}]}
        ]"#,
    )
    .unwrap();
    let description = abi.describe();
    assert!(
        description
            .functions
            .iter()
            .any(|item| item.signature == "read(address)")
    );
    assert!(
        description
            .functions
            .iter()
            .any(|item| item.signature == "read(uint256)")
    );
    assert_ne!(
        &abi.encode_call("read(address)", &[Address::ZERO.to_string()])
            .unwrap()[..4],
        &abi.encode_call("read(uint256)", &["0".to_owned()]).unwrap()[..4]
    );

    let mut word = [0_u8; 32];
    word[31] = 42;
    assert_eq!(
        abi.decode_output("read(uint256)", &word).unwrap(),
        vec!["42"]
    );

    let mut revert = keccak256("Unauthorized(address)").as_slice()[..4].to_vec();
    revert.extend_from_slice(&[0_u8; 12]);
    revert.extend_from_slice(Address::with_last_byte(7).as_slice());
    assert!(
        abi.decode_revert(&revert)
            .unwrap()
            .starts_with("Unauthorized(")
    );

    let mut standard = vec![0x08, 0xc3, 0x79, 0xa0];
    let mut encoded = [0_u8; 96];
    encoded[31] = 32;
    encoded[63] = 2;
    encoded[64..66].copy_from_slice(b"no");
    standard.extend_from_slice(&encoded);
    assert_eq!(abi.decode_revert(&standard).as_deref(), Some("Error(no)"));
}

#[test]
fn two_account_proxy_delegatecall_reads_proxy_storage() {
    let proxy = Address::with_last_byte(0x81);
    let implementation = Address::with_last_byte(0x82);
    let implementation_code = alloy_primitives::hex::decode("60005460005260206000f3").unwrap();
    let mut proxy_code = alloy_primitives::hex::decode("36600060003760006000366000").unwrap();
    proxy_code.push(0x73);
    proxy_code.extend_from_slice(implementation.as_slice());
    proxy_code
        .extend_from_slice(&alloy_primitives::hex::decode("5af4503d600060003e3d6000f3").unwrap());
    let key = keccak256(B256::ZERO);

    let mut workspace = SnapshotWorkspace::new();
    workspace
        .add_batch(&snapshot(proxy, &proxy_code, vec![(key, U256::from(99))]))
        .unwrap();
    workspace
        .add_batch(&snapshot(implementation, &implementation_code, vec![]))
        .unwrap();
    let result = workspace.execute(call(proxy, "0x")).unwrap();
    assert_eq!(result.status, "success");
    assert_eq!(output_word(&result.output), U256::from(99));
}

#[test]
fn wasm_api_returns_typed_json_envelopes() {
    let executor = crate::SnapshotExecutor::new();
    let accounts: serde_json::Value = serde_json::from_str(&executor.accounts()).unwrap();
    assert_eq!(accounts["ok"], true);
    assert_eq!(accounts["data"].as_array().unwrap().len(), 0);

    let invalid: serde_json::Value = serde_json::from_str(&executor.execute("not json")).unwrap();
    assert_eq!(invalid["ok"], false);
    assert_eq!(invalid["error"]["code"], "invalid-request");
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn exported_executor_constructs() {
        let executor = crate::SnapshotExecutor::new();
        let response: serde_json::Value = serde_json::from_str(&executor.accounts()).unwrap();
        assert_eq!(response["ok"], true);
    }
}

use super::*;
use alloy_primitives::{Address, B256, Bytes, U256, keccak256};

fn account_record(address: Address, nonce: u64) -> AccountRecord {
    let code = Bytes::from_static(&[0x60, 0x00]);
    let storage = StorageEntries::new(vec![(B256::with_last_byte(1), U256::from(7))]).unwrap();
    AccountRecord {
        account: AccountSnapshot {
            address,
            nonce,
            balance: U256::from(9),
            code_hash: keccak256(&code),
            storage_root: storage.root().unwrap(),
        },
        code,
        storage,
    }
}

fn fixture() -> Snapshot {
    let record = account_record(Address::with_last_byte(6), 1);
    Snapshot {
        version: VERSION,
        execution: ExecutionContext {
            chain_id: 1,
            hardfork: EvmHardfork::Prague,
            blob_base_fee_update_fraction: Some(3_338_477),
        },
        block: BlockContext {
            number: 1,
            hash: B256::with_last_byte(2),
            parent_hash: B256::with_last_byte(1),
            state_root: B256::with_last_byte(3),
            timestamp: 10,
            beneficiary: Address::with_last_byte(4),
            gas_limit: 30_000_000,
            base_fee_per_gas: Some(1),
            difficulty: U256::ZERO,
            prev_randao: Some(B256::with_last_byte(5)),
            excess_blob_gas: Some(0),
            slot_number: None,
        },
        block_hashes: BlockHashes::new(vec![(0, B256::with_last_byte(1))]).unwrap(),
        accounts: vec![record],
    }
}

#[test]
fn v1_round_trip_is_valid() {
    let snapshot = fixture();
    let encoded = snapshot.to_vec().unwrap();
    let decoded = Snapshot::from_slice(&encoded).unwrap();
    assert_eq!(decoded, snapshot);
}

#[test]
fn v1_round_trip_with_multiple_accounts() {
    let mut snapshot = fixture();
    let address_a = Address::with_last_byte(6);
    let address_b = Address::with_last_byte(7);
    snapshot.accounts = vec![account_record(address_a, 1), account_record(address_b, 2)];
    let encoded = snapshot.to_vec().unwrap();
    let decoded = Snapshot::from_slice(&encoded).unwrap();
    assert_eq!(decoded, snapshot);
}

#[test]
fn rejects_wrong_version() {
    let mut snapshot = fixture();
    snapshot.version = 2;
    assert!(matches!(
        snapshot.validate(),
        Err(SnapshotFormatError::UnsupportedVersion { actual: 2 })
    ));
}

#[test]
fn serialized_as_single_canonical_json_object() {
    let snapshot = fixture();
    let bytes = snapshot.to_vec().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["version"], 1);
    assert!(value["accounts"].is_array());
    assert_eq!(value["accounts"].as_array().unwrap().len(), 1);
    // Ensure no flat single-account fields remain at the top level.
    assert!(value.get("account").is_none());
    assert!(value.get("code").is_none());
    assert!(value.get("storage").is_none());
}

#[test]
fn rejects_empty_account_list() {
    let mut snapshot = fixture();
    snapshot.accounts = Vec::new();
    assert!(matches!(
        snapshot.validate(),
        Err(SnapshotFormatError::EmptyAccountList)
    ));
}

#[test]
fn rejects_duplicate_account_addresses() {
    let address = Address::with_last_byte(6);
    let mut snapshot = fixture();
    snapshot.accounts = vec![
        account_record(address, 1),
        account_record(Address::with_last_byte(7), 2),
        account_record(address, 3),
    ];
    assert!(matches!(
        snapshot.validate(),
        Err(SnapshotFormatError::DuplicateAccountAddress(a)) if a == address
    ));
}

#[test]
fn rejects_non_canonical_quantities() {
    let mut value = String::from_utf8(fixture().to_vec().unwrap()).unwrap();
    value = value.replacen("\"chainId\":\"0x1\"", "\"chainId\":\"0x01\"", 1);
    assert!(Snapshot::from_slice(value.as_bytes()).is_err());
}

#[test]
fn rejects_duplicate_and_unsorted_storage() {
    let duplicate = StorageEntries::new(vec![
        (B256::with_last_byte(1), U256::from(1)),
        (B256::with_last_byte(1), U256::from(2)),
    ]);
    assert!(matches!(
        duplicate,
        Err(SnapshotFormatError::DuplicateStorageKey(_))
    ));

    let unsorted = StorageEntries::new(vec![
        (B256::with_last_byte(2), U256::from(1)),
        (B256::with_last_byte(1), U256::from(2)),
    ]);
    assert!(matches!(
        unsorted,
        Err(SnapshotFormatError::StorageKeysOutOfOrder { .. })
    ));
}

#[test]
fn validates_code_and_storage_roots() {
    let mut bad_code = fixture();
    bad_code.accounts[0].code = Bytes::from_static(&[0x00]);
    assert!(matches!(
        bad_code.validate(),
        Err(SnapshotFormatError::BytecodeHashMismatch { .. })
    ));

    let mut bad_storage = fixture();
    bad_storage.accounts[0].account.storage_root = B256::ZERO;
    assert!(matches!(
        bad_storage.validate(),
        Err(SnapshotFormatError::StorageRootMismatch { .. })
    ));
}

#[test]
fn requires_the_complete_block_hash_window() {
    let mut snapshot = fixture();
    snapshot.block_hashes = BlockHashes::default();
    assert!(matches!(
        snapshot.validate(),
        Err(SnapshotFormatError::BlockHashWindowLength { .. })
    ));
}

#[test]
fn rejects_duplicate_block_hash_numbers() {
    let hashes = BlockHashes::new(vec![
        (1, B256::with_last_byte(1)),
        (1, B256::with_last_byte(2)),
    ]);
    assert!(matches!(
        hashes,
        Err(SnapshotFormatError::DuplicateBlockNumber(1))
    ));
}

#[test]
fn rejects_execution_fields_from_the_wrong_fork() {
    let mut snapshot = fixture();
    snapshot.execution.hardfork = EvmHardfork::Berlin;
    assert!(matches!(
        snapshot.validate(),
        Err(SnapshotFormatError::UnexpectedForkField { .. })
    ));
}

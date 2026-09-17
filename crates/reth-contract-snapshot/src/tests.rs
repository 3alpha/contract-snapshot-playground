use super::*;
use alloy_primitives::{Address, B256, Bytes, U256, keccak256};
use snapshot_format::{
    AccountSnapshot, BlockContext, BlockHashes, EvmHardfork, ExecutionContext, SnapshotFormatError,
    StorageEntries,
};
use std::{cell::RefCell, collections::VecDeque, error::Error, fmt};

fn fixture_anchor() -> SnapshotAnchor {
    SnapshotAnchor {
        execution: ExecutionContext {
            chain_id: 1,
            hardfork: EvmHardfork::Prague,
            blob_base_fee_update_fraction: Some(3_338_477),
        },
        block: BlockContext {
            number: 2,
            hash: B256::repeat_byte(0x42),
            parent_hash: B256::with_last_byte(1),
            state_root: B256::repeat_byte(0x43),
            timestamp: 1_700_000_000,
            beneficiary: Address::repeat_byte(0x44),
            gas_limit: 30_000_000,
            base_fee_per_gas: Some(7),
            difficulty: U256::ZERO,
            prev_randao: Some(B256::repeat_byte(0x45)),
            excess_blob_gas: Some(0),
            slot_number: None,
        },
        block_hashes: BlockHashes::new(vec![(0, B256::ZERO), (1, B256::with_last_byte(1))])
            .unwrap(),
    }
}

fn fixture_account(address: Address, code: Bytes, entries: &[(B256, U256)]) -> SnapshotAccount {
    let storage = StorageEntries::new(entries.to_vec()).unwrap();
    SnapshotAccount {
        account: AccountSnapshot {
            address,
            nonce: 1,
            balance: U256::from(2),
            code_hash: keccak256(&code),
            storage_root: storage.root().unwrap(),
        },
        code,
    }
}

struct FakeSource {
    expected_anchor: SnapshotAnchor,
    accounts: Vec<SnapshotAccount>,
    pages: RefCell<VecDeque<Result<StoragePage, TestSourceError>>>,
    seen_anchors: RefCell<Vec<SnapshotAnchor>>,
    seen_starts: RefCell<Vec<B256>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestSourceError {
    AccountMissing,
    UnexpectedPageRequest,
}

impl fmt::Display for TestSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountMissing => formatter.write_str("account does not exist"),
            Self::UnexpectedPageRequest => formatter.write_str("unexpected extra page request"),
        }
    }
}

impl Error for TestSourceError {}

impl FakeSource {
    fn new(accounts: Vec<SnapshotAccount>, pages: Vec<StoragePage>) -> Self {
        Self {
            expected_anchor: fixture_anchor(),
            accounts,
            pages: RefCell::new(pages.into_iter().map(Ok).collect()),
            seen_anchors: RefCell::new(Vec::new()),
            seen_starts: RefCell::new(Vec::new()),
        }
    }

    fn one(account: SnapshotAccount, pages: Vec<StoragePage>) -> Self {
        Self::new(vec![account], pages)
    }
}

impl ContractStateSource for FakeSource {
    type Error = TestSourceError;

    fn anchor(&self) -> Result<SnapshotAnchor, Self::Error> {
        Ok(self.expected_anchor.clone())
    }

    fn anchor_at(&self, block_number: u64) -> Result<SnapshotAnchor, Self::Error> {
        if block_number == self.expected_anchor.block.number {
            Ok(self.expected_anchor.clone())
        } else {
            Err(TestSourceError::UnexpectedPageRequest)
        }
    }

    fn account(
        &self,
        anchor: &SnapshotAnchor,
        address: Address,
    ) -> Result<SnapshotAccount, Self::Error> {
        self.seen_anchors.borrow_mut().push(anchor.clone());
        self.accounts
            .iter()
            .find(|candidate| candidate.address() == address)
            .cloned()
            .ok_or(TestSourceError::AccountMissing)
    }

    fn storage_range(
        &self,
        anchor: &SnapshotAnchor,
        _: Address,
        start: B256,
        _: usize,
    ) -> Result<StoragePage, Self::Error> {
        self.seen_anchors.borrow_mut().push(anchor.clone());
        self.seen_starts.borrow_mut().push(start);
        self.pages
            .borrow_mut()
            .pop_front()
            .unwrap_or(Err(TestSourceError::UnexpectedPageRequest))
    }
}

#[test]
fn address_hashing_uses_keccak256() {
    let address: Address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
        .parse()
        .unwrap();
    assert_eq!(
        keccak256(address),
        "0x7b5855bb92cd7f3f78137497df02f6ccb9badda93d9782e0f230c807ba728be0"
            .parse::<B256>()
            .unwrap()
    );
}

#[test]
fn pagination_advances_to_the_successor_without_duplicates() {
    let address = Address::repeat_byte(0x11);
    let first = (B256::with_last_byte(1), U256::from(10));
    let second = (B256::with_last_byte(2), U256::from(20));
    let account = fixture_account(address, Bytes::new(), &[first, second]);
    let source = FakeSource::one(
        account.clone(),
        vec![
            StoragePage {
                entries: vec![first],
                end: StorageRangeEnd::ByteLimit,
            },
            StoragePage {
                entries: vec![second],
                end: StorageRangeEnd::Exhausted,
            },
        ],
    );
    let mut output = Vec::new();
    let stats = write_accounts(
        &source,
        &fixture_anchor(),
        &[account],
        64,
        &mut output,
        |_| {},
    )
    .unwrap();
    assert_eq!(
        stats,
        SnapshotStats {
            pages: 2,
            entries: 2
        }
    );
    assert_eq!(
        *source.seen_starts.borrow(),
        vec![B256::ZERO, B256::with_last_byte(2)]
    );
    let snapshot = snapshot_format::Snapshot::from_slice(&output).unwrap();
    assert_eq!(snapshot.accounts.len(), 1);
    assert_eq!(snapshot.accounts[0].account.address, address);
}

#[test]
fn multiple_accounts_share_one_envelope_and_anchor() {
    let address_a = Address::repeat_byte(0x11);
    let address_b = Address::repeat_byte(0x22);
    let entry_a = (B256::with_last_byte(1), U256::from(10));
    let entry_b = (B256::with_last_byte(2), U256::from(20));
    let account_a = fixture_account(address_a, Bytes::from_static(&[0x60]), &[entry_a]);
    let account_b = fixture_account(address_b, Bytes::from_static(&[0x61]), &[entry_b]);
    let source = FakeSource::new(
        vec![account_a.clone(), account_b.clone()],
        vec![
            StoragePage {
                entries: vec![entry_a],
                end: StorageRangeEnd::Exhausted,
            },
            StoragePage {
                entries: vec![entry_b],
                end: StorageRangeEnd::Exhausted,
            },
        ],
    );
    let anchor = fixture_anchor();
    let mut output = Vec::new();
    let stats = write_accounts(
        &source,
        &anchor,
        &[account_a, account_b],
        64,
        &mut output,
        |_| {},
    )
    .unwrap();
    assert_eq!(
        stats,
        SnapshotStats {
            pages: 2,
            entries: 2
        }
    );

    let snapshot = snapshot_format::Snapshot::from_slice(&output).unwrap();
    assert_eq!(snapshot.version, snapshot_format::VERSION);
    assert_eq!(snapshot.accounts.len(), 2);
    assert_eq!(snapshot.accounts[0].account.address, address_a);
    assert_eq!(snapshot.accounts[1].account.address, address_b);
    assert_eq!(snapshot.block, anchor.block);
    assert!(
        source
            .seen_anchors
            .borrow()
            .iter()
            .all(|seen| seen == &anchor)
    );
}

#[test]
fn repeated_account_addresses_are_rejected_before_storage_is_read() {
    let address = Address::repeat_byte(0x11);
    let account = fixture_account(address, Bytes::new(), &[]);
    let source = FakeSource::one(account.clone(), vec![]);

    assert!(matches!(
        write_accounts(
            &source,
            &fixture_anchor(),
            &[account.clone(), account],
            64,
            Vec::new(),
            |_| {},
        ),
        Err(SnapshotError::Format(SnapshotFormatError::DuplicateAccountAddress(duplicate)))
            if duplicate == address
    ));
    assert!(source.seen_starts.borrow().is_empty());
}

#[test]
fn duplicate_storage_key_fails() {
    let address = Address::repeat_byte(0x11);
    let key = B256::with_last_byte(1);
    let account = fixture_account(address, Bytes::new(), &[(key, U256::from(1))]);
    let source = FakeSource::one(
        account.clone(),
        vec![StoragePage {
            entries: vec![(key, U256::from(1)), (key, U256::from(2))],
            end: StorageRangeEnd::Exhausted,
        }],
    );
    assert!(matches!(
        write_accounts(
            &source,
            &fixture_anchor(),
            &[account],
            64,
            Vec::new(),
            |_| {}
        ),
        Err(SnapshotError::DuplicateStorageKey { .. })
    ));
}

#[test]
fn pagination_requires_explicit_exhaustion() {
    let address = Address::repeat_byte(0x11);
    let entry = (B256::repeat_byte(0xff), U256::from(1));
    let account = fixture_account(address, Bytes::new(), &[entry]);
    let source = FakeSource::one(
        account.clone(),
        vec![StoragePage {
            entries: vec![entry],
            end: StorageRangeEnd::HashLimit,
        }],
    );
    assert!(matches!(
        write_accounts(
            &source,
            &fixture_anchor(),
            &[account],
            64,
            Vec::new(),
            |_| {}
        ),
        Err(SnapshotError::MaximumKeyWithoutExhaustion)
    ));
}

#[test]
fn validates_bytecode_and_nonexistent_accounts() {
    let code = [0x60, 0x00, 0x56];
    assert!(validate_bytecode(&code, keccak256(code)).is_ok());
    assert!(validate_bytecode(&code, B256::ZERO).is_err());

    let source = FakeSource::new(vec![], vec![]);
    assert!(matches!(
        source.account(&fixture_anchor(), Address::ZERO),
        Err(TestSourceError::AccountMissing)
    ));
}

#[test]
fn every_operation_stays_pinned_to_one_anchor() {
    let address = Address::repeat_byte(0x11);
    let account = fixture_account(address, Bytes::new(), &[]);
    let source = FakeSource::one(
        account,
        vec![StoragePage {
            entries: vec![],
            end: StorageRangeEnd::Exhausted,
        }],
    );
    let chosen = source.anchor().unwrap();
    let returned = source.account(&chosen, address).unwrap();
    write_accounts(&source, &chosen, &[returned], 64, Vec::new(), |_| {}).unwrap();
    assert!(
        source
            .seen_anchors
            .borrow()
            .iter()
            .all(|seen| seen == &chosen)
    );
}

use crate::{
    ContractStateSource, PageProgress, SnapshotAccount, SnapshotAnchor, SnapshotError,
    SnapshotStats, StorageRangeEnd,
};
use alloy_primitives::{B256, U256, keccak256};
use snapshot_format::{
    AccountRecord, Snapshot, SnapshotFormatError, StorageEntries, StorageRootBuilder, VERSION,
};
use std::{collections::BTreeSet, io::Write};

/// Collects every requested account at one shared anchor and writes a single v1 envelope.
///
/// Storage pagination continues per account against the same anchor; once every account is
/// exhausted and root-validated, the complete envelope is serialized as one canonical JSON object.
pub fn write_accounts<S, W, P>(
    source: &S,
    anchor: &SnapshotAnchor,
    accounts: &[SnapshotAccount],
    page_bytes: usize,
    mut output: W,
    mut progress: P,
) -> Result<SnapshotStats, SnapshotError>
where
    S: ContractStateSource,
    W: Write,
    P: FnMut(PageProgress),
{
    if page_bytes == 0 {
        return Err(SnapshotError::InvalidPageBytes);
    }
    validate_account_addresses(accounts)?;
    anchor.block_hashes.validate(&anchor.block)?;
    anchor.execution.validate_for_block(&anchor.block)?;

    let mut stats = SnapshotStats::default();
    let mut records = Vec::with_capacity(accounts.len());
    for account in accounts {
        let record = collect_account(
            source,
            anchor,
            account,
            page_bytes,
            &mut stats,
            &mut progress,
        )?;
        records.push(record);
    }

    let snapshot = Snapshot {
        version: VERSION,
        execution: anchor.execution.clone(),
        block: anchor.block.clone(),
        block_hashes: anchor.block_hashes.clone(),
        accounts: records,
    };
    serde_json::to_writer(&mut output, &snapshot)?;
    output.flush()?;
    Ok(stats)
}

fn validate_account_addresses(accounts: &[SnapshotAccount]) -> Result<(), SnapshotError> {
    if accounts.is_empty() {
        return Err(SnapshotFormatError::EmptyAccountList.into());
    }

    let mut unique = BTreeSet::new();
    for account in accounts {
        let address = account.address();
        if !unique.insert(address) {
            return Err(SnapshotFormatError::DuplicateAccountAddress(address).into());
        }
    }
    Ok(())
}

fn collect_account<S, P>(
    source: &S,
    anchor: &SnapshotAnchor,
    account: &SnapshotAccount,
    page_bytes: usize,
    stats: &mut SnapshotStats,
    progress: &mut P,
) -> Result<AccountRecord, SnapshotError>
where
    S: ContractStateSource,
    P: FnMut(PageProgress),
{
    validate_bytecode(&account.code, account.account.code_hash)?;

    let mut start = B256::ZERO;
    let mut previous_key = None;
    let mut root_builder = StorageRootBuilder::new();
    let mut entries = Vec::new();

    loop {
        let page_number = stats.pages + 1;
        let page = source
            .storage_range(anchor, account.address(), start, page_bytes)
            .map_err(|source| SnapshotError::StoragePage {
                page: page_number,
                source: Box::new(source),
            })?;
        stats.pages += 1;

        validate_page_shape(&page.entries, page.end)?;
        for (key, value) in &page.entries {
            validate_entry(*key, *value, start, previous_key)?;
            root_builder.push(*key, *value)?;
            entries.push((*key, *value));
            previous_key = Some(*key);
            stats.entries = stats
                .entries
                .checked_add(1)
                .ok_or(SnapshotError::EntryCountOverflow)?;
        }

        progress(PageProgress {
            page: stats.pages,
            entries: page.entries.len(),
            total_entries: stats.entries,
            end: page.end,
        });

        match page.end {
            StorageRangeEnd::Exhausted => break,
            StorageRangeEnd::HashLimit => return Err(hash_limit_error(&page.entries)),
            StorageRangeEnd::ByteLimit => {
                start = next_page_start(&page.entries, start)?;
            }
        }
    }

    let actual_root = root_builder.finish();
    if actual_root != account.account.storage_root {
        return Err(SnapshotError::StorageRootMismatch {
            expected: account.account.storage_root,
            actual: actual_root,
        });
    }

    let storage = StorageEntries::new(entries)?;
    Ok(AccountRecord {
        account: account.account.clone(),
        code: account.code.clone(),
        storage,
    })
}

pub fn validate_bytecode(code: &[u8], expected_hash: B256) -> Result<(), SnapshotError> {
    let actual_hash = keccak256(code);
    if actual_hash != expected_hash {
        return Err(SnapshotError::BytecodeHashMismatch {
            expected: expected_hash,
            actual: actual_hash,
        });
    }
    Ok(())
}

fn validate_page_shape(
    entries: &[(B256, U256)],
    end: StorageRangeEnd,
) -> Result<(), SnapshotError> {
    if entries.is_empty() && end != StorageRangeEnd::Exhausted {
        return Err(SnapshotError::EmptyNonExhaustedPage);
    }
    Ok(())
}

fn validate_entry(
    key: B256,
    value: U256,
    start: B256,
    previous_key: Option<B256>,
) -> Result<(), SnapshotError> {
    if value.is_zero() {
        return Err(SnapshotError::ZeroStorageValue { key });
    }
    if let Some(previous) = previous_key {
        if key == previous {
            return Err(SnapshotError::DuplicateStorageKey { key });
        }
        if key < previous {
            return Err(SnapshotError::StorageKeysOutOfOrder {
                previous,
                current: key,
            });
        }
    }
    if key < start {
        return Err(SnapshotError::StorageKeyBeforeStart { key, start });
    }
    Ok(())
}

fn hash_limit_error(entries: &[(B256, U256)]) -> SnapshotError {
    let Some((last, _)) = entries.last() else {
        return SnapshotError::EmptyHashLimitedPage;
    };
    if *last != B256::repeat_byte(0xff) {
        return SnapshotError::HashLimitBeforeMaximum { last: *last };
    }
    SnapshotError::MaximumKeyWithoutExhaustion
}

fn next_page_start(entries: &[(B256, U256)], start: B256) -> Result<B256, SnapshotError> {
    let last = entries
        .last()
        .map(|entry| entry.0)
        .ok_or(SnapshotError::EmptyByteLimitedPage)?;
    let next = key_successor(last).ok_or(SnapshotError::ContinuationOverflow { last })?;
    if next <= start {
        return Err(SnapshotError::NoPaginationProgress { start, next });
    }
    Ok(next)
}

pub fn key_successor(key: B256) -> Option<B256> {
    let mut bytes = key.0;
    for byte in bytes.iter_mut().rev() {
        let (next, overflow) = byte.overflowing_add(1);
        *byte = next;
        if !overflow {
            return Some(B256::from(bytes));
        }
    }
    None
}

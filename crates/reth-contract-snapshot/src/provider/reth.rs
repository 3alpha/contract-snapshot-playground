use super::{ProviderResultExt, RethSourceError};
use crate::{ContractStateSource, SnapshotAccount, SnapshotAnchor, StoragePage, StorageRangeEnd};
use alloy_primitives::{Address, B256, Bytes, keccak256};
use reth_chainspec::{ChainSpec, EthChainSpec, MAINNET};
use reth_db::DatabaseEnv;
use reth_ethereum_engine_primitives::EthEngineTypes;
use reth_ethereum_primitives::EthPrimitives;
use reth_node_types::{NodeTypes, NodeTypesWithDBAdapter};
use reth_provider::{
    ProviderFactory,
    providers::{BlockchainProvider, ProviderFactoryBuilder, ReadOnlyConfig},
};
use reth_storage_api::{
    AccountReader, BlockHashReader, BlockNumReader, BytecodeReader, EthStorage, HeaderProvider,
    RangeEnd, StateProviderFactory, StateRangeProviderFactory, StateRangeView, StorageRootProvider,
};
use reth_tasks::Runtime;
use revm::primitives::hardfork::SpecId;
use snapshot_format::{AccountSnapshot, BlockContext, BlockHashes, EvmHardfork, ExecutionContext};
use std::path::Path;

const ACCOUNT_RANGE_BYTES: usize = 1024;
const MAX_HASH: B256 = B256::repeat_byte(0xff);

#[derive(Clone, Debug)]
struct SnapshotNode;

impl NodeTypes for SnapshotNode {
    type Primitives = EthPrimitives;
    type ChainSpec = ChainSpec;
    type Storage = EthStorage;
    type Payload = EthEngineTypes;
}

type EthereumDbTypes = NodeTypesWithDBAdapter<SnapshotNode, DatabaseEnv>;
type EthereumProviderFactory = ProviderFactory<EthereumDbTypes>;
type EthereumBlockchainProvider = BlockchainProvider<EthereumDbTypes>;

#[derive(Debug)]
pub struct RethStateSource {
    factory: EthereumProviderFactory,
    blockchain: EthereumBlockchainProvider,
}

impl RethStateSource {
    /// Opens MDBX without an exclusive lock, static files read-only, and RocksDB as a secondary.
    pub fn open(datadir: &Path) -> Result<Self, RethSourceError> {
        if !datadir.is_dir() {
            return Err(RethSourceError::InvalidDatadir(datadir.to_path_buf()));
        }
        let factory = ProviderFactoryBuilder::<SnapshotNode>::default()
            .open_read_only(
                MAINNET.clone(),
                ReadOnlyConfig::from_datadir(datadir),
                Runtime::test(),
            )
            .map_err(|source| RethSourceError::OpenDatadir {
                path: datadir.to_path_buf(),
                reason: source.to_string(),
            })?;
        let blockchain = BlockchainProvider::new(factory.clone())
            .with_operation("constructing the blockchain provider")?;
        Ok(Self {
            factory,
            blockchain,
        })
    }

    fn verify_anchor(&self, anchor: &SnapshotAnchor) -> Result<(), RethSourceError> {
        let provider = self
            .factory
            .provider()
            .with_operation("opening an anchor verification transaction")?;
        let number = anchor.block_number();
        let header = provider
            .sealed_header(number)
            .with_operation("reading the canonical anchor header")?
            .ok_or(RethSourceError::AnchorHeaderUnavailable(number))?;
        if header.hash() != anchor.block_hash() {
            return Err(RethSourceError::AnchorNotCanonical {
                number,
                expected: anchor.block_hash(),
                actual: header.hash(),
            });
        }
        if header.state_root != anchor.state_root() {
            return Err(RethSourceError::AnchorStateRootChanged {
                number,
                expected: anchor.state_root(),
                actual: header.state_root,
            });
        }
        Ok(())
    }

    fn pinned_view(&self, anchor: &SnapshotAnchor) -> Result<StateRangeView, RethSourceError> {
        self.verify_anchor(anchor)?;
        self.blockchain
            .state_range_provider(anchor.state_root())
            .with_operation("resolving the pinned state root")?
            .ok_or(RethSourceError::StateRootNotRetained(anchor.state_root()))
    }

    fn resolve_anchor(
        &self,
        requested_number: Option<u64>,
    ) -> Result<SnapshotAnchor, RethSourceError> {
        let provider = self
            .factory
            .provider()
            .with_operation("opening the anchor transaction")?;
        let chain_info = provider
            .chain_info()
            .with_operation("reading canonical chain information")?;
        let block_number = requested_number.unwrap_or(chain_info.best_number);
        if block_number > chain_info.best_number {
            return Err(RethSourceError::RequestedBlockAhead {
                requested: block_number,
                latest: chain_info.best_number,
            });
        }
        let header = provider
            .sealed_header(block_number)
            .with_operation("reading the canonical snapshot header")?
            .ok_or(RethSourceError::CanonicalHeaderMissing(block_number))?;
        if block_number == chain_info.best_number && header.hash() != chain_info.best_hash {
            return Err(RethSourceError::CanonicalHeadMismatch {
                number: chain_info.best_number,
                expected: chain_info.best_hash,
                actual: header.hash(),
            });
        }

        let history_start = block_number.saturating_sub(256);
        let historical_hashes = provider
            .canonical_hashes_range(history_start, block_number)
            .with_operation("reading the canonical BLOCKHASH window")?;
        let expected_hashes = usize::try_from(block_number.min(256)).unwrap_or(256);
        if historical_hashes.len() != expected_hashes {
            return Err(RethSourceError::BlockHashWindowIncomplete {
                expected: expected_hashes,
                actual: historical_hashes.len(),
            });
        }
        let numbered_hashes = historical_hashes
            .into_iter()
            .enumerate()
            .map(|(offset, hash)| {
                let offset = u64::try_from(offset).unwrap_or(0);
                (history_start + offset, hash)
            })
            .collect();
        let block_hashes = BlockHashes::new(numbered_hashes)?;

        let spec_id = alloy_evm::spec(MAINNET.as_ref(), header.header());
        let hardfork = hardfork_from_spec_id(spec_id);
        let blob_fields = if spec_id >= SpecId::CANCUN {
            let parameters = MAINNET
                .blob_params_at_timestamp(header.timestamp)
                .ok_or(RethSourceError::MissingBlobParameters(block_number))?;
            (header.excess_blob_gas, Some(parameters.update_fraction))
        } else {
            (None, None)
        };

        let anchor = SnapshotAnchor {
            execution: ExecutionContext {
                chain_id: MAINNET.chain_id(),
                hardfork,
                blob_base_fee_update_fraction: blob_fields.1,
            },
            block: BlockContext {
                number: header.number,
                hash: header.hash(),
                parent_hash: header.parent_hash,
                state_root: header.state_root,
                timestamp: header.timestamp,
                beneficiary: header.beneficiary,
                gas_limit: header.gas_limit,
                base_fee_per_gas: header.base_fee_per_gas,
                difficulty: header.difficulty,
                prev_randao: (spec_id >= SpecId::MERGE).then_some(header.mix_hash),
                excess_blob_gas: blob_fields.0,
                slot_number: (spec_id >= SpecId::AMSTERDAM)
                    .then_some(header.slot_number)
                    .flatten(),
            },
            block_hashes,
        };
        anchor.block_hashes.validate(&anchor.block)?;
        anchor.execution.validate_for_block(&anchor.block)?;
        Ok(anchor)
    }
}

impl ContractStateSource for RethStateSource {
    type Error = RethSourceError;

    fn anchor(&self) -> Result<SnapshotAnchor, Self::Error> {
        self.resolve_anchor(None)
    }

    fn anchor_at(&self, block_number: u64) -> Result<SnapshotAnchor, Self::Error> {
        self.resolve_anchor(Some(block_number))
    }

    fn account(
        &self,
        anchor: &SnapshotAnchor,
        address: Address,
    ) -> Result<SnapshotAccount, Self::Error> {
        let hashed_address = keccak256(address);
        let range_view = self.pinned_view(anchor)?;
        let ranged = range_view
            .account_range(hashed_address, hashed_address, ACCOUNT_RANGE_BYTES)
            .with_operation("reading the hashed account range")?;
        let Some((returned_hash, range_account)) = ranged.items.into_iter().next() else {
            return Err(RethSourceError::AccountNotFound {
                address,
                block: anchor.block_number(),
            });
        };
        if returned_hash != hashed_address {
            return Err(RethSourceError::AccountNotFound {
                address,
                block: anchor.block_number(),
            });
        }

        let range_storage_root = range_view
            .storage_root_by_hash(hashed_address)
            .with_operation("computing the hashed storage root")?;
        drop(range_view);

        let state = self
            .blockchain
            .history_by_block_hash(anchor.block_hash())
            .with_operation("resolving the block-pinned state")?;
        let state_account = state
            .basic_account(&address)
            .with_operation("reading the block-pinned account")?
            .ok_or(RethSourceError::AccountDisappeared(address))?;
        if state_account != range_account {
            return Err(RethSourceError::AccountMismatch { address });
        }

        let state_storage_root = state
            .storage_root(address, Default::default())
            .with_operation("computing the block-pinned storage root")?;
        if state_storage_root != range_storage_root {
            return Err(RethSourceError::StorageRootMismatch {
                address,
                range_root: range_storage_root,
                state_root: state_storage_root,
            });
        }

        let empty_code_hash = keccak256([]);
        let code_hash = state_account.bytecode_hash.unwrap_or(empty_code_hash);
        let code = if code_hash == empty_code_hash {
            Bytes::new()
        } else {
            state
                .bytecode_by_hash(&code_hash)
                .with_operation("reading account bytecode")?
                .ok_or(RethSourceError::BytecodeMissing(code_hash))?
                .original_bytes()
        };
        drop(state);
        self.verify_anchor(anchor)?;

        Ok(SnapshotAccount {
            account: AccountSnapshot {
                address,
                nonce: state_account.nonce,
                balance: state_account.balance,
                code_hash,
                storage_root: range_storage_root,
            },
            code,
        })
    }

    fn storage_range(
        &self,
        anchor: &SnapshotAnchor,
        address: Address,
        start: B256,
        page_bytes: usize,
    ) -> Result<StoragePage, Self::Error> {
        let hashed_address = keccak256(address);
        let view = self.pinned_view(anchor)?;
        let response = view
            .storage_range(hashed_address, start, MAX_HASH, page_bytes)
            .with_operation("reading a hashed storage range")?
            .ok_or(RethSourceError::AccountAbsentFromTrie {
                address,
                state_root: anchor.state_root(),
            })?;
        drop(view);
        self.verify_anchor(anchor)?;

        let end = match response.end {
            RangeEnd::Exhausted => StorageRangeEnd::Exhausted,
            RangeEnd::HashLimit => StorageRangeEnd::HashLimit,
            RangeEnd::ByteLimit => StorageRangeEnd::ByteLimit,
        };
        Ok(StoragePage {
            entries: response.items,
            end,
        })
    }
}

fn hardfork_from_spec_id(spec_id: SpecId) -> EvmHardfork {
    match spec_id {
        SpecId::FRONTIER => EvmHardfork::Frontier,
        SpecId::HOMESTEAD => EvmHardfork::Homestead,
        SpecId::TANGERINE => EvmHardfork::Tangerine,
        SpecId::SPURIOUS_DRAGON => EvmHardfork::SpuriousDragon,
        SpecId::BYZANTIUM => EvmHardfork::Byzantium,
        SpecId::PETERSBURG => EvmHardfork::Petersburg,
        SpecId::ISTANBUL => EvmHardfork::Istanbul,
        SpecId::BERLIN => EvmHardfork::Berlin,
        SpecId::LONDON => EvmHardfork::London,
        SpecId::MERGE => EvmHardfork::Paris,
        SpecId::SHANGHAI => EvmHardfork::Shanghai,
        SpecId::CANCUN => EvmHardfork::Cancun,
        SpecId::PRAGUE => EvmHardfork::Prague,
        SpecId::OSAKA => EvmHardfork::Osaka,
        SpecId::AMSTERDAM => EvmHardfork::Amsterdam,
    }
}

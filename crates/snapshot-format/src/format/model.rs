use super::{BlockHashes, SnapshotFormatError, StorageEntries, VERSION, hex};
use alloy_primitives::{Address, B256, Bytes, U256, keccak256};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum EvmHardfork {
    Frontier,
    Homestead,
    Tangerine,
    SpuriousDragon,
    Byzantium,
    Constantinople,
    Petersburg,
    Istanbul,
    MuirGlacier,
    Berlin,
    London,
    ArrowGlacier,
    GrayGlacier,
    Paris,
    Shanghai,
    Cancun,
    Prague,
    Osaka,
    Amsterdam,
}

impl EvmHardfork {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Frontier => "frontier",
            Self::Homestead => "homestead",
            Self::Tangerine => "tangerine",
            Self::SpuriousDragon => "spuriousDragon",
            Self::Byzantium => "byzantium",
            Self::Constantinople => "constantinople",
            Self::Petersburg => "petersburg",
            Self::Istanbul => "istanbul",
            Self::MuirGlacier => "muirGlacier",
            Self::Berlin => "berlin",
            Self::London => "london",
            Self::ArrowGlacier => "arrowGlacier",
            Self::GrayGlacier => "grayGlacier",
            Self::Paris => "paris",
            Self::Shanghai => "shanghai",
            Self::Cancun => "cancun",
            Self::Prague => "prague",
            Self::Osaka => "osaka",
            Self::Amsterdam => "amsterdam",
        }
    }
}

impl fmt::Display for EvmHardfork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for EvmHardfork {
    type Err = SnapshotFormatError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "frontier" => Ok(Self::Frontier),
            "homestead" => Ok(Self::Homestead),
            "tangerine" => Ok(Self::Tangerine),
            "spuriousDragon" => Ok(Self::SpuriousDragon),
            "byzantium" => Ok(Self::Byzantium),
            "constantinople" => Ok(Self::Constantinople),
            "petersburg" => Ok(Self::Petersburg),
            "istanbul" => Ok(Self::Istanbul),
            "muirGlacier" => Ok(Self::MuirGlacier),
            "berlin" => Ok(Self::Berlin),
            "london" => Ok(Self::London),
            "arrowGlacier" => Ok(Self::ArrowGlacier),
            "grayGlacier" => Ok(Self::GrayGlacier),
            "paris" => Ok(Self::Paris),
            "shanghai" => Ok(Self::Shanghai),
            "cancun" => Ok(Self::Cancun),
            "prague" => Ok(Self::Prague),
            "osaka" => Ok(Self::Osaka),
            "amsterdam" => Ok(Self::Amsterdam),
            _ => Err(SnapshotFormatError::UnknownHardfork(value.to_owned())),
        }
    }
}

impl Serialize for EvmHardfork {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EvmHardfork {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionContext {
    #[serde(
        serialize_with = "hex::quantity_u64",
        deserialize_with = "hex::quantity_u64_from"
    )]
    pub chain_id: u64,
    pub hardfork: EvmHardfork,
    #[serde(
        serialize_with = "hex::quantity_u128_option",
        deserialize_with = "hex::quantity_u128_option_from"
    )]
    pub blob_base_fee_update_fraction: Option<u128>,
}

impl ExecutionContext {
    pub fn validate_for_block(&self, block: &BlockContext) -> Result<(), SnapshotFormatError> {
        if block.gas_limit == 0 {
            return Err(SnapshotFormatError::InvalidContextField {
                field: "gasLimit",
                reason: "must be greater than zero",
            });
        }
        if self.blob_base_fee_update_fraction == Some(0) {
            return Err(SnapshotFormatError::InvalidContextField {
                field: "blobBaseFeeUpdateFraction",
                reason: "must be greater than zero",
            });
        }
        validate_fork_fields(self, block)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlockContext {
    #[serde(
        serialize_with = "hex::quantity_u64",
        deserialize_with = "hex::quantity_u64_from"
    )]
    pub number: u64,
    #[serde(
        serialize_with = "hex::fixed_b256",
        deserialize_with = "hex::fixed_b256_from"
    )]
    pub hash: B256,
    #[serde(
        serialize_with = "hex::fixed_b256",
        deserialize_with = "hex::fixed_b256_from"
    )]
    pub parent_hash: B256,
    #[serde(
        serialize_with = "hex::fixed_b256",
        deserialize_with = "hex::fixed_b256_from"
    )]
    pub state_root: B256,
    #[serde(
        serialize_with = "hex::quantity_u64",
        deserialize_with = "hex::quantity_u64_from"
    )]
    pub timestamp: u64,
    #[serde(
        serialize_with = "hex::fixed_address",
        deserialize_with = "hex::fixed_address_from"
    )]
    pub beneficiary: Address,
    #[serde(
        serialize_with = "hex::quantity_u64",
        deserialize_with = "hex::quantity_u64_from"
    )]
    pub gas_limit: u64,
    #[serde(
        serialize_with = "hex::quantity_u64_option",
        deserialize_with = "hex::quantity_u64_option_from"
    )]
    pub base_fee_per_gas: Option<u64>,
    #[serde(
        serialize_with = "hex::quantity_u256",
        deserialize_with = "hex::quantity_u256_from"
    )]
    pub difficulty: U256,
    #[serde(
        serialize_with = "hex::fixed_b256_option",
        deserialize_with = "hex::fixed_b256_option_from"
    )]
    pub prev_randao: Option<B256>,
    #[serde(
        serialize_with = "hex::quantity_u64_option",
        deserialize_with = "hex::quantity_u64_option_from"
    )]
    pub excess_blob_gas: Option<u64>,
    #[serde(
        serialize_with = "hex::quantity_u64_option",
        deserialize_with = "hex::quantity_u64_option_from"
    )]
    pub slot_number: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountSnapshot {
    #[serde(
        serialize_with = "hex::fixed_address",
        deserialize_with = "hex::fixed_address_from"
    )]
    pub address: Address,
    #[serde(
        serialize_with = "hex::quantity_u64",
        deserialize_with = "hex::quantity_u64_from"
    )]
    pub nonce: u64,
    #[serde(
        serialize_with = "hex::quantity_u256",
        deserialize_with = "hex::quantity_u256_from"
    )]
    pub balance: U256,
    #[serde(
        serialize_with = "hex::fixed_b256",
        deserialize_with = "hex::fixed_b256_from"
    )]
    pub code_hash: B256,
    #[serde(
        serialize_with = "hex::fixed_b256",
        deserialize_with = "hex::fixed_b256_from"
    )]
    pub storage_root: B256,
}

/// One account's complete persistent state inside a [`Snapshot`] batch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccountRecord {
    pub account: AccountSnapshot,
    #[serde(serialize_with = "hex::bytes", deserialize_with = "hex::bytes_from")]
    pub code: Bytes,
    pub storage: StorageEntries,
}

impl AccountRecord {
    pub fn validate(&self) -> Result<(), SnapshotFormatError> {
        let actual_code_hash = keccak256(&self.code);
        if actual_code_hash != self.account.code_hash {
            return Err(SnapshotFormatError::BytecodeHashMismatch {
                expected: self.account.code_hash,
                actual: actual_code_hash,
            });
        }

        self.storage.validate()?;
        let actual_storage_root = self.storage.root()?;
        if actual_storage_root != self.account.storage_root {
            return Err(SnapshotFormatError::StorageRootMismatch {
                expected: self.account.storage_root,
                actual: actual_storage_root,
            });
        }
        Ok(())
    }
}

/// Canonical v1 multi-account snapshot envelope.
///
/// The execution, block, and block-hash context is shared by every account record in the file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub version: u64,
    pub execution: ExecutionContext,
    pub block: BlockContext,
    pub block_hashes: BlockHashes,
    pub accounts: Vec<AccountRecord>,
}

impl Snapshot {
    pub fn from_slice(bytes: &[u8]) -> Result<Self, SnapshotFormatError> {
        let snapshot: Self = serde_json::from_slice(bytes)?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn to_vec(&self) -> Result<Vec<u8>, SnapshotFormatError> {
        self.validate()?;
        serde_json::to_vec(self).map_err(SnapshotFormatError::from)
    }

    pub fn validate(&self) -> Result<(), SnapshotFormatError> {
        if self.version != VERSION {
            return Err(SnapshotFormatError::UnsupportedVersion {
                actual: self.version,
            });
        }

        if self.accounts.is_empty() {
            return Err(SnapshotFormatError::EmptyAccountList);
        }

        let mut addresses = BTreeSet::new();
        for record in &self.accounts {
            let address = record.account.address;
            if !addresses.insert(address) {
                return Err(SnapshotFormatError::DuplicateAccountAddress(address));
            }
            record.validate()?;
        }

        self.block_hashes.validate(&self.block)?;
        self.execution.validate_for_block(&self.block)
    }
}

fn validate_fork_fields(
    execution: &ExecutionContext,
    block: &BlockContext,
) -> Result<(), SnapshotFormatError> {
    validate_optional_field(
        execution.hardfork,
        EvmHardfork::London,
        "baseFeePerGas",
        block.base_fee_per_gas.is_some(),
    )?;
    validate_optional_field(
        execution.hardfork,
        EvmHardfork::Paris,
        "prevRandao",
        block.prev_randao.is_some(),
    )?;
    validate_optional_field(
        execution.hardfork,
        EvmHardfork::Cancun,
        "excessBlobGas",
        block.excess_blob_gas.is_some(),
    )?;
    validate_optional_field(
        execution.hardfork,
        EvmHardfork::Cancun,
        "blobBaseFeeUpdateFraction",
        execution.blob_base_fee_update_fraction.is_some(),
    )?;
    validate_optional_field(
        execution.hardfork,
        EvmHardfork::Amsterdam,
        "slotNumber",
        block.slot_number.is_some(),
    )
}

fn validate_optional_field(
    hardfork: EvmHardfork,
    introduced: EvmHardfork,
    field: &'static str,
    present: bool,
) -> Result<(), SnapshotFormatError> {
    if hardfork >= introduced && !present {
        return Err(SnapshotFormatError::MissingForkField {
            hardfork: hardfork.as_str(),
            field,
        });
    }
    if hardfork < introduced && present {
        return Err(SnapshotFormatError::UnexpectedForkField {
            hardfork: hardfork.as_str(),
            field,
        });
    }
    Ok(())
}

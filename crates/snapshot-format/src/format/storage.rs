use super::{BlockContext, SnapshotFormatError, hex};
use alloy_primitives::{B256, U256};
use alloy_trie::{HashBuilder, Nibbles};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _, ser::SerializeMap};
use std::fmt;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StorageEntries(Vec<(B256, U256)>);

impl StorageEntries {
    pub fn new(entries: Vec<(B256, U256)>) -> Result<Self, SnapshotFormatError> {
        let storage = Self(entries);
        storage.validate()?;
        Ok(storage)
    }

    pub fn as_slice(&self) -> &[(B256, U256)] {
        &self.0
    }

    pub fn get(&self, key: &B256) -> Option<U256> {
        self.0
            .binary_search_by_key(key, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.0[index].1)
    }

    pub fn validate(&self) -> Result<(), SnapshotFormatError> {
        let mut previous = None;
        for (key, value) in &self.0 {
            if value.is_zero() {
                return Err(SnapshotFormatError::ZeroStorageValue(*key));
            }
            if let Some(previous_key) = previous {
                if *key == previous_key {
                    return Err(SnapshotFormatError::DuplicateStorageKey(*key));
                }
                if *key < previous_key {
                    return Err(SnapshotFormatError::StorageKeysOutOfOrder {
                        previous: previous_key,
                        current: *key,
                    });
                }
            }
            previous = Some(*key);
        }
        Ok(())
    }

    pub fn root(&self) -> Result<B256, SnapshotFormatError> {
        let mut builder = StorageRootBuilder::new();
        for (key, value) in &self.0 {
            builder.push(*key, *value)?;
        }
        Ok(builder.finish())
    }
}

impl Serialize for StorageEntries {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(&key.to_string(), &format!("{value:#x}"))?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for StorageEntries {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StorageVisitor;

        impl<'de> serde::de::Visitor<'de> for StorageVisitor {
            type Value = StorageEntries;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an ordered object of hashed storage keys and non-zero values")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some((key, value)) = map.next_entry::<String, String>()? {
                    let key = hex::parse_b256("storage key", &key).map_err(M::Error::custom)?;
                    let value = hex::parse_quantity_u256("storage value", &value)
                        .map_err(M::Error::custom)?;
                    entries.push((key, value));
                }
                StorageEntries::new(entries).map_err(M::Error::custom)
            }
        }

        deserializer.deserialize_map(StorageVisitor)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BlockHashes(Vec<(u64, B256)>);

impl BlockHashes {
    pub fn new(entries: Vec<(u64, B256)>) -> Result<Self, SnapshotFormatError> {
        let hashes = Self(entries);
        hashes.validate_order()?;
        Ok(hashes)
    }

    pub fn as_slice(&self) -> &[(u64, B256)] {
        &self.0
    }

    pub fn get(&self, number: u64) -> Option<B256> {
        self.0
            .binary_search_by_key(&number, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.0[index].1)
    }

    pub fn validate(&self, block: &BlockContext) -> Result<(), SnapshotFormatError> {
        self.validate_order()?;
        let expected_len = usize::try_from(block.number.min(256)).unwrap_or(256);
        if self.0.len() != expected_len {
            return Err(SnapshotFormatError::BlockHashWindowLength {
                expected: expected_len,
                actual: self.0.len(),
            });
        }

        let start = block.number.saturating_sub(256);
        for (offset, (actual, _)) in self.0.iter().enumerate() {
            let expected = start + u64::try_from(offset).unwrap_or(0);
            if *actual != expected {
                return Err(SnapshotFormatError::BlockHashNumber {
                    expected,
                    actual: *actual,
                });
            }
        }

        if let Some((_, actual_parent)) = self.0.last()
            && *actual_parent != block.parent_hash
        {
            return Err(SnapshotFormatError::ParentHashMismatch {
                expected: block.parent_hash,
                actual: *actual_parent,
            });
        }
        Ok(())
    }

    fn validate_order(&self) -> Result<(), SnapshotFormatError> {
        let mut previous = None;
        for (current, _) in &self.0 {
            if let Some(previous_number) = previous {
                if *current == previous_number {
                    return Err(SnapshotFormatError::DuplicateBlockNumber(*current));
                }
                if *current < previous_number {
                    return Err(SnapshotFormatError::BlockNumbersOutOfOrder {
                        previous: previous_number,
                        current: *current,
                    });
                }
            }
            previous = Some(*current);
        }
        Ok(())
    }
}

impl Serialize for BlockHashes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (number, hash) in &self.0 {
            map.serialize_entry(&format!("{number:#x}"), &hash.to_string())?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for BlockHashes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BlockHashesVisitor;

        impl<'de> serde::de::Visitor<'de> for BlockHashesVisitor {
            type Value = BlockHashes;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an ordered object of block numbers and hashes")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: serde::de::MapAccess<'de>,
            {
                let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some((number, hash)) = map.next_entry::<String, String>()? {
                    let number = hex::parse_quantity_u64("block hash number", &number)
                        .map_err(M::Error::custom)?;
                    let hash = hex::parse_b256("block hash", &hash).map_err(M::Error::custom)?;
                    entries.push((number, hash));
                }
                BlockHashes::new(entries).map_err(M::Error::custom)
            }
        }

        deserializer.deserialize_map(BlockHashesVisitor)
    }
}

#[derive(Debug, Default)]
pub struct StorageRootBuilder {
    builder: HashBuilder,
    previous: Option<B256>,
}

impl StorageRootBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, key: B256, value: U256) -> Result<(), SnapshotFormatError> {
        if value.is_zero() {
            return Err(SnapshotFormatError::ZeroStorageValue(key));
        }
        if let Some(previous) = self.previous {
            if key == previous {
                return Err(SnapshotFormatError::DuplicateStorageKey(key));
            }
            if key < previous {
                return Err(SnapshotFormatError::StorageKeysOutOfOrder {
                    previous,
                    current: key,
                });
            }
        }

        let encoded = alloy_rlp::encode_fixed_size(&value);
        self.builder
            .add_leaf_unchecked(Nibbles::unpack(key), encoded.as_ref());
        self.previous = Some(key);
        Ok(())
    }

    pub fn finish(mut self) -> B256 {
        self.builder.root()
    }
}

use crate::SnapshotFormatError;
use alloy_primitives::{Address, B256, Bytes, U256};
use serde::{Deserialize, Deserializer, Serializer, de::Error as _};
use std::str::FromStr;

pub(crate) fn quantity_u64<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{value:#x}"))
}

pub(crate) fn quantity_u64_from<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    parse_quantity_u64("quantity", &value).map_err(D::Error::custom)
}

pub(crate) fn quantity_u128_option<S>(
    value: &Option<u128>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serializer.serialize_some(&format!("{value:#x}")),
        None => serializer.serialize_none(),
    }
}

pub(crate) fn quantity_u128_option_from<'de, D>(deserializer: D) -> Result<Option<u128>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_quantity_u128("quantity", &value).map_err(D::Error::custom))
        .transpose()
}

pub(crate) fn quantity_u64_option<S>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serializer.serialize_some(&format!("{value:#x}")),
        None => serializer.serialize_none(),
    }
}

pub(crate) fn quantity_u64_option_from<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_quantity_u64("quantity", &value).map_err(D::Error::custom))
        .transpose()
}

pub(crate) fn quantity_u256<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("{value:#x}"))
}

pub(crate) fn quantity_u256_from<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    parse_quantity_u256("quantity", &value).map_err(D::Error::custom)
}

pub(crate) fn fixed_b256<S>(value: &B256, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string())
}

pub(crate) fn fixed_b256_from<'de, D>(deserializer: D) -> Result<B256, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    parse_b256("hash", &value).map_err(D::Error::custom)
}

pub(crate) fn fixed_b256_option<S>(value: &Option<B256>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serializer.serialize_some(&value.to_string()),
        None => serializer.serialize_none(),
    }
}

pub(crate) fn fixed_b256_option_from<'de, D>(deserializer: D) -> Result<Option<B256>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    value
        .map(|value| parse_b256("hash", &value).map_err(D::Error::custom))
        .transpose()
}

pub(crate) fn fixed_address<S>(value: &Address, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&value.to_string().to_lowercase())
}

pub(crate) fn fixed_address_from<'de, D>(deserializer: D) -> Result<Address, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    parse_address("address", &value).map_err(D::Error::custom)
}

pub(crate) fn bytes<S>(value: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&format!("0x{}", alloy_primitives::hex::encode(value)))
}

pub(crate) fn bytes_from<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    parse_bytes("code", &value).map_err(D::Error::custom)
}

pub(crate) fn parse_quantity_u64(
    field: &'static str,
    value: &str,
) -> Result<u64, SnapshotFormatError> {
    let parsed = parse_quantity_u256(field, value)?;
    parsed
        .try_into()
        .map_err(|_| SnapshotFormatError::QuantityOverflow {
            field,
            value: value.to_owned(),
        })
}

pub(crate) fn parse_quantity_u128(
    field: &'static str,
    value: &str,
) -> Result<u128, SnapshotFormatError> {
    let parsed = parse_quantity_u256(field, value)?;
    parsed
        .try_into()
        .map_err(|_| SnapshotFormatError::QuantityOverflow {
            field,
            value: value.to_owned(),
        })
}

pub(crate) fn parse_quantity_u256(
    field: &'static str,
    value: &str,
) -> Result<U256, SnapshotFormatError> {
    let Some(hex) = value.strip_prefix("0x") else {
        return Err(non_canonical_quantity(field, value));
    };
    if hex.is_empty()
        || (hex.len() > 1 && hex.starts_with('0'))
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(non_canonical_quantity(field, value));
    }
    U256::from_str_radix(hex, 16).map_err(|_| SnapshotFormatError::QuantityOverflow {
        field,
        value: value.to_owned(),
    })
}

pub(crate) fn parse_b256(field: &'static str, value: &str) -> Result<B256, SnapshotFormatError> {
    validate_fixed_hex(field, value, 32)?;
    B256::from_str(value).map_err(|_| SnapshotFormatError::InvalidFixedHex {
        field,
        value: value.to_owned(),
        bytes: 32,
    })
}

pub(crate) fn parse_address(
    field: &'static str,
    value: &str,
) -> Result<Address, SnapshotFormatError> {
    validate_fixed_hex(field, value, 20)?;
    Address::from_str(value).map_err(|_| SnapshotFormatError::InvalidFixedHex {
        field,
        value: value.to_owned(),
        bytes: 20,
    })
}

pub(crate) fn parse_bytes(field: &'static str, value: &str) -> Result<Bytes, SnapshotFormatError> {
    let Some(hex) = value.strip_prefix("0x") else {
        return Err(SnapshotFormatError::InvalidBytesHex {
            field,
            value: value.to_owned(),
        });
    };
    if hex.len() % 2 != 0
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SnapshotFormatError::InvalidBytesHex {
            field,
            value: value.to_owned(),
        });
    }
    alloy_primitives::hex::decode(hex)
        .map(Bytes::from)
        .map_err(|_| SnapshotFormatError::InvalidBytesHex {
            field,
            value: value.to_owned(),
        })
}

fn validate_fixed_hex(
    field: &'static str,
    value: &str,
    bytes: usize,
) -> Result<(), SnapshotFormatError> {
    let valid = value.len() == 2 + bytes * 2
        && value.starts_with("0x")
        && value[2..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if !valid {
        return Err(SnapshotFormatError::InvalidFixedHex {
            field,
            value: value.to_owned(),
            bytes,
        });
    }
    Ok(())
}

fn non_canonical_quantity(field: &'static str, value: &str) -> SnapshotFormatError {
    SnapshotFormatError::NonCanonicalQuantity {
        field,
        value: value.to_owned(),
    }
}

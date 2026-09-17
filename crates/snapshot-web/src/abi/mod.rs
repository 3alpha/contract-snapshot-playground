mod error;

pub use error::AbiError;

use alloy_dyn_abi::{DynSolType, DynSolValue, FunctionExt, JsonAbiExt, Specifier};
use alloy_json_abi::{Function, JsonAbi};
use alloy_primitives::hex;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbiParameter {
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbiFunction {
    pub signature: String,
    pub name: String,
    pub state_mutability: String,
    pub inputs: Vec<AbiParameter>,
    pub outputs: Vec<AbiParameter>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbiDescription {
    pub functions: Vec<AbiFunction>,
}

#[derive(Clone, Debug)]
pub struct ContractAbi {
    abi: JsonAbi,
}

impl ContractAbi {
    pub fn parse(json: &str) -> Result<Self, AbiError> {
        let abi = JsonAbi::from_json_str(json).map_err(AbiError::Json)?;
        Ok(Self { abi })
    }

    pub fn describe(&self) -> AbiDescription {
        let mut functions: Vec<_> = self
            .abi
            .functions()
            .map(|function| AbiFunction {
                signature: function.signature(),
                name: function.name.clone(),
                state_mutability: function.state_mutability.as_json_str().to_owned(),
                inputs: function.inputs.iter().map(parameter_description).collect(),
                outputs: function.outputs.iter().map(parameter_description).collect(),
            })
            .collect();
        functions.sort_by(|left, right| left.signature.cmp(&right.signature));
        AbiDescription { functions }
    }

    pub fn encode_call(&self, signature: &str, arguments: &[String]) -> Result<Vec<u8>, AbiError> {
        let function = self.function(signature)?;
        if arguments.len() != function.inputs.len() {
            return Err(AbiError::ArgumentCount {
                signature: signature.to_owned(),
                expected: function.inputs.len(),
                actual: arguments.len(),
            });
        }

        let values = function
            .inputs
            .iter()
            .zip(arguments)
            .enumerate()
            .map(|(index, (parameter, value))| {
                let kind = parameter
                    .resolve()
                    .map_err(|source| AbiError::ResolveType {
                        parameter: parameter.name.clone(),
                        reason: source.to_string(),
                    })?;
                kind.coerce_str(value)
                    .map_err(|source| AbiError::InvalidArgument {
                        signature: signature.to_owned(),
                        index,
                        value: value.clone(),
                        reason: source.to_string(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        function
            .abi_encode_input(&values)
            .map_err(|source| AbiError::Encode {
                signature: signature.to_owned(),
                reason: source.to_string(),
            })
    }

    pub fn decode_output(&self, signature: &str, data: &[u8]) -> Result<Vec<String>, AbiError> {
        let function = self.function(signature)?;
        function
            .abi_decode_output(data)
            .map(|values| values.iter().map(display_value).collect())
            .map_err(|source| AbiError::DecodeOutput {
                signature: signature.to_owned(),
                reason: source.to_string(),
            })
    }

    pub fn decode_revert(&self, data: &[u8]) -> Option<String> {
        if let Some(reason) = Self::decode_standard_revert(data) {
            return Some(reason);
        }
        if data.len() < 4 {
            return None;
        }
        for error in self.abi.errors() {
            if data[..4] == error.selector()[..] {
                let decoded = error.abi_decode_input(&data[4..]).ok()?;
                let values = decoded
                    .iter()
                    .map(display_value)
                    .collect::<Vec<_>>()
                    .join(", ");
                return Some(format!("{}({values})", error.name));
            }
        }
        None
    }

    pub(crate) fn decode_standard_revert(data: &[u8]) -> Option<String> {
        if data.len() < 4 {
            return None;
        }
        if data[..4] == [0x08, 0xc3, 0x79, 0xa0] {
            return DynSolType::String
                .abi_decode(&data[4..])
                .ok()
                .map(|value| format!("Error({})", display_value(&value)));
        }
        if data[..4] == [0x4e, 0x48, 0x7b, 0x71] {
            return DynSolType::Uint(256)
                .abi_decode(&data[4..])
                .ok()
                .map(|value| format!("Panic({})", display_value(&value)));
        }
        None
    }

    fn function(&self, signature: &str) -> Result<&Function, AbiError> {
        self.abi
            .functions()
            .find(|function| function.signature() == signature)
            .ok_or_else(|| AbiError::FunctionNotFound(signature.to_owned()))
    }
}

fn parameter_description(parameter: &alloy_json_abi::Param) -> AbiParameter {
    AbiParameter {
        name: parameter.name.clone(),
        kind: parameter
            .resolve()
            .map(|kind| kind.to_string())
            .unwrap_or_else(|_| parameter.ty.clone()),
    }
}

fn display_value(value: &DynSolValue) -> String {
    match value {
        DynSolValue::Bool(value) => value.to_string(),
        DynSolValue::Int(value, _) => value.to_string(),
        DynSolValue::Uint(value, _) => value.to_string(),
        DynSolValue::FixedBytes(value, size) => {
            format!("0x{}", hex::encode(&value[..*size]))
        }
        DynSolValue::Address(value) => value.to_string().to_lowercase(),
        DynSolValue::Function(value) => format!("0x{}", hex::encode(value)),
        DynSolValue::Bytes(value) => format!("0x{}", hex::encode(value)),
        DynSolValue::String(value) => value.clone(),
        DynSolValue::Array(values) | DynSolValue::FixedArray(values) => {
            let values = values
                .iter()
                .map(display_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        DynSolValue::Tuple(values) => {
            let values = values
                .iter()
                .map(display_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({values})")
        }
    }
}

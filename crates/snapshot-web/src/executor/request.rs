use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum CallInput {
    Raw {
        calldata: String,
    },
    Abi {
        signature: String,
        arguments: Vec<String>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
    pub target: String,
    pub caller: String,
    pub value: String,
    pub gas_limit: u64,
    #[serde(flatten)]
    pub input: CallInput,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedAccountSummary {
    pub address: String,
    pub nonce: String,
    pub balance: String,
    pub code_size: usize,
    pub storage_entries: usize,
    pub block_number: String,
    pub block_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallResult {
    pub status: String,
    pub gas_used: u64,
    pub calldata: String,
    pub output: String,
    pub decoded: Option<Vec<String>>,
    pub reason: Option<String>,
}

mod error;

use error::ApiError;
use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::{CallRequest, SnapshotWorkspace};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiResponse<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiFailure>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiFailure {
    code: &'static str,
    message: String,
    causes: Vec<String>,
}

#[wasm_bindgen]
pub struct SnapshotExecutor {
    workspace: SnapshotWorkspace,
}

#[wasm_bindgen]
impl SnapshotExecutor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            workspace: SnapshotWorkspace::new(),
        }
    }

    #[wasm_bindgen(js_name = addBatch)]
    pub fn add_batch(&mut self, snapshot: &[u8]) -> String {
        serialize(self.workspace.add_batch(snapshot).map_err(ApiError::from))
    }

    #[wasm_bindgen(js_name = removeSnapshot)]
    pub fn remove_snapshot(&mut self, address: &str) -> String {
        serialize(
            self.workspace
                .remove_snapshot(address)
                .map_err(ApiError::from),
        )
    }

    pub fn accounts(&self) -> String {
        serialize::<_, ApiError>(Ok(self.workspace.accounts()))
    }

    #[wasm_bindgen(js_name = setAbi)]
    pub fn set_abi(&mut self, address: &str, json: &str) -> String {
        serialize(
            self.workspace
                .set_abi(address, json)
                .map_err(ApiError::from),
        )
    }

    #[wasm_bindgen(js_name = removeAbi)]
    pub fn remove_abi(&mut self, address: &str) -> String {
        serialize(self.workspace.remove_abi(address).map_err(ApiError::from))
    }

    pub fn execute(&self, request_json: &str) -> String {
        let result = serde_json::from_str::<CallRequest>(request_json)
            .map_err(ApiError::RequestJson)
            .and_then(|request| self.workspace.execute(request).map_err(ApiError::from));
        serialize(result)
    }
}

impl Default for SnapshotExecutor {
    fn default() -> Self {
        Self::new()
    }
}

fn serialize<T, E>(result: Result<T, E>) -> String
where
    T: Serialize,
    E: Into<ApiError>,
{
    let response = match result {
        Ok(data) => ApiResponse {
            ok: true,
            data: Some(data),
            error: None,
        },
        Err(error) => {
            let error = error.into();
            ApiResponse {
                ok: false,
                data: None,
                error: Some(ApiFailure {
                    code: error.code(),
                    message: error.to_string(),
                    causes: error.causes(),
                }),
            }
        }
    };
    serde_json::to_string(&response).unwrap_or_else(|_| {
        "{\"ok\":false,\"data\":null,\"error\":{\"code\":\"serialization\",\"message\":\"failed to serialize worker response\",\"causes\":[]}}".to_owned()
    })
}

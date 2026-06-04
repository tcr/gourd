//! Go's `json` package helpers.
//!
//! Provides 2 JSON operations matching Go's stdlib.

/// Go's `json.Marshal(v)` — marshals a value to JSON bytes.
pub fn json_marshal<T: serde::Serialize>(v: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(v).map_err(|e| e.to_string())
}

/// Go's `json.Unmarshal(data, v)` — unmarshals JSON bytes into a value.
pub fn json_unmarshal<T: serde::de::DeserializeOwned>(data: &[u8]) -> Result<T, String> {
    serde_json::from_slice(data).map_err(|e| e.to_string())
}

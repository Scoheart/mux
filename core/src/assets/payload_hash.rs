use serde::Serialize;
use sha2::{Digest, Sha256};

const PREFIX: &str = "json-v1:";

/// Hash JSON content rather than a HashMap's randomized iteration order.
/// Object keys are sorted recursively; array order and all values are preserved.
pub(super) fn hash(value: &impl Serialize) -> Result<String, String> {
    let mut value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    value.sort_all_objects();
    let bytes = serde_json::to_vec(&value).map_err(|error| error.to_string())?;
    Ok(format!("{PREFIX}{}", hex::encode(Sha256::digest(bytes))))
}

pub(super) fn matches(value: &impl Serialize, expected: &str) -> Result<bool, String> {
    if expected.starts_with(PREFIX) {
        return hash(value).map(|actual| actual == expected);
    }
    // Existing persisted plans retain their original hash contract. Never
    // accept a legacy mismatch merely because the hashing scheme changed.
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    Ok(hex::encode(Sha256::digest(bytes)) == expected)
}

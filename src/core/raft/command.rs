use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Increment,
    Decrement,
    IncrementBy(u64),
    DecrementBy(u64),
    Set(u64),
}

impl Command {
    pub fn to_bytes(&self) -> Vec<u8> {
        let bytes = serde_json::to_vec(self).unwrap();
        debug!("Serialized command: {:?}", self);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let command = serde_json::from_slice(bytes)?;
        debug!("Deserialized command: {:?}", command);
        Ok(command)
    }
}

#[cfg(test)]
mod tests {
    // ... (Your round-trip serialization tests for all variants)
}

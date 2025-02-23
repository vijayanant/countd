use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    Increment,
    Decrement,
    IncrementBy(u64),
    DecrementBy(u64),
    Set(u64),
}


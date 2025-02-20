use tracing::instrument;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counter {
    pub value: u64,
}

impl Counter {
    pub fn new(initial_value: u64) -> Self {
        tracing::info!("Creating a new counter with initial value: {}", initial_value);
        Self { value: initial_value }
    }

    #[instrument(name = "increment_counter", level = "debug")]
    pub fn increment(&mut self) {
        self.value += 1;
        tracing::info!("Counter incremented to: {}", self.value);
    }
}

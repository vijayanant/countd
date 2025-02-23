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

    #[instrument(name = "decrement_counter", level = "debug")]
    pub fn decrement(&mut self) {
        if self.value > 0 {
            self.value -= 1;
            tracing::info!("Counter decremented to: {}", self.value);
        } else {
            tracing::warn!("Counter is already at 0, cannot decrement further.");
        }
    }

    #[instrument(name = "increment_counter_by", level = "debug")]
    pub fn increment_by(&mut self, amount: u64) {
        self.value += amount;
        tracing::info!("Counter incremented by {} to: {}", amount, self.value);
    }

    #[instrument(name = "decrement_counter_by", level = "debug")]
    pub fn decrement_by(&mut self, amount: u64) {
        if self.value >= amount {
            self.value -= amount;
            tracing::info!("Counter decremented by {} to: {}", amount, self.value);
        } else {
            tracing::warn!("Counter value is less than decrement amount, setting to 0.");
            self.value = 0;
        }
    }

    #[instrument(name = "set_counter", level = "debug")]
    pub fn set(&mut self, new_value: u64) {
        self.value = new_value;
        tracing::info!("Counter set to: {}", self.value);
    }
}

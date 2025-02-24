use tracing::instrument;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counter {
    pub value: u64,
}

#[derive(Error, Debug, PartialEq)]
pub enum CounterError {
    #[error("Counter is already at 0, cannot decrement further.")]
    DecrementUnderflow,
    #[error("Counter value is less than decrement amount, setting to 0.")]
    DecrementByUnderflow,
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
    pub fn decrement(&mut self) -> Result<(), CounterError> {
        if self.value > 0 {
            self.value -= 1;
            tracing::info!("Counter decremented to: {}", self.value);
            Ok(())
        } else {
            tracing::error!("Counter is already at 0, cannot decrement further.");
            Err(CounterError::DecrementUnderflow)
        }
    }

    #[instrument(name = "increment_counter_by", level = "debug")]
    pub fn increment_by(&mut self, amount: u64) {
        self.value += amount;
        tracing::info!("Counter incremented by {} to: {}", amount, self.value);
    }

    #[instrument(name = "decrement_counter_by", level = "debug")]
    pub fn decrement_by(&mut self, amount: u64) -> Result<(), CounterError> {
        if self.value >= amount {
            self.value -= amount;
            tracing::info!("Counter decremented by {} to: {}", amount, self.value);
            Ok(())
        } else {
            tracing::error!("Counter value is less than decrement amount");
            Err(CounterError::DecrementByUnderflow)
        }
    }

    #[instrument(name = "set_counter", level = "debug")]
    pub fn set(&mut self, new_value: u64) {
        self.value = new_value;
        tracing::info!("Counter set to: {}", self.value);
    }
}

#[cfg(test)]
mod tests {
    use crate::core::counter::{Counter, CounterError};

    #[test]
    fn test_increment() {
        let mut counter = Counter::new(0);
        counter.increment();
        assert_eq!(counter.value, 1);
    }

    #[test]
    fn test_decrement() {
        let mut counter = Counter::new(1);
        counter.decrement().unwrap();
        assert_eq!(counter.value, 0);
    }

    #[test]
    fn test_decrement_underflow() {
        let mut counter = Counter::new(0);
        assert_eq!(counter.decrement(), Err(CounterError::DecrementUnderflow));
    }

    #[test]
    fn test_increment_by() {
        let mut counter = Counter::new(5);
        counter.increment_by(3);
        assert_eq!(counter.value, 8);
    }

    #[test]
    fn test_decrement_by() {
        let mut counter = Counter::new(10);
        counter.decrement_by(4).unwrap();
        assert_eq!(counter.value, 6);
    }

    #[test]
    fn test_decrement_by_underflow() {
        let mut counter = Counter::new(2);
        assert_eq!(counter.decrement_by(5), Err(CounterError::DecrementByUnderflow));
    }

    #[test]
    fn test_set() {
        let mut counter = Counter::new(0);
        counter.set(100);
        assert_eq!(counter.value, 100);
    }
}

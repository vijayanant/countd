#[derive(Debug, Clone, PartialEq, Eq)] // Add useful traits
pub struct Counter {
    pub value: u64,
}

impl Counter {
    pub fn new(initial_value: u64) -> Self {
        Self { value: initial_value }
    }

    pub fn increment(&mut self) {
        self.value += 1;
    }
}



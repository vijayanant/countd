#[cfg(test)]
mod tests {
    use crate::core::counter::Counter;

    #[test]
    fn test_increment() {
        let mut counter = Counter::new(0);
        counter.increment();
        assert_eq!(counter.value, 1);

        counter.increment();
        assert_eq!(counter.value, 2);
    }

    #[test]
    fn test_new() {
        let counter = Counter::new(42);
        assert_eq!(counter.value, 42);
    }
}

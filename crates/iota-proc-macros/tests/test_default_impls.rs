use iota_proc_macros::{extend_impl_with_non_fallible, extend_trait_with_non_fallible};

#[extend_trait_with_non_fallible("Default implementation failed")]
pub trait SimpleService {
    // Method with no default implementation
    fn try_get(&self, key: &str) -> Result<String, String>;

    // Method with default implementation
    fn try_multi_get(&self, keys: &[String]) -> Result<Vec<String>, String> {
        let mut results = Vec::new();
        for key in keys {
            match self.try_get(key) {
                Ok(value) => results.push(value),
                Err(e) => return Err(e),
            }
        }
        Ok(results)
    }
}

pub struct MockService;

#[extend_impl_with_non_fallible("Mock service operation failed")]
impl SimpleService for MockService {
    fn try_get(&self, key: &str) -> Result<String, String> {
        Ok(format!("Value for {}", key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_methods() {
        let service = MockService;

        // Test non-fallible method generated from regular method
        let value = service.get("test-key");
        assert_eq!(value, "Value for test-key");

        // Test non-fallible method generated from default implementation
        let values = service.multi_get(&["key1".to_string(), "key2".to_string()]);
        assert_eq!(values, vec!["Value for key1", "Value for key2"]);
    }
}

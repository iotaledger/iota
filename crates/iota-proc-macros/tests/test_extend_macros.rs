// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
#[cfg(test)]
mod tests {
    use std::future::Future;

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

    // Mock IotaResult type for testing
    type IotaResult<T> = Result<T, Box<dyn std::error::Error>>;

    #[extend_trait_with_non_fallible("Default implementation failed")]
    pub trait ExtendedService {
        // Test with standard Result
        fn try_get(&self, key: &str) -> Result<String, String>;

        // Test with IotaResult
        fn try_put(&self, key: &str, value: String) -> IotaResult<()>;

        // Test with Future<Output = Result<...>>
        fn try_get_async(&self, key: &str) -> impl Future<Output = Result<String, String>>;

        // Test with Future<Output = IotaResult<...>>
        fn try_put_async(&self, key: &str, value: String) -> impl Future<Output = IotaResult<()>>;

        // Non try_ methods should be ignored
        fn list_keys(&self) -> Vec<String>;
    }

    pub struct MockStore;

    #[extend_impl_with_non_fallible("Mock store operation failed")]
    impl ExtendedService for MockStore {
        fn try_get(&self, _key: &str) -> Result<String, String> {
            Ok("test_value".to_string())
        }

        fn try_put(&self, _key: &str, _value: String) -> IotaResult<()> {
            Ok(())
        }

        fn try_get_async(&self, _key: &str) -> impl Future<Output = Result<String, String>> {
            async { Ok("async_value".to_string()) }
        }

        fn try_put_async(
            &self,
            _key: &str,
            _value: String,
        ) -> impl Future<Output = IotaResult<()>> {
            async { Ok(()) }
        }

        fn list_keys(&self) -> Vec<String> {
            vec!["key1".to_string(), "key2".to_string()]
        }
    }

    #[tokio::test]
    async fn test_extended_methods() {
        let store = MockStore;

        // Test direct Result methods
        let value = store.get("test_key");
        assert_eq!(value, "test_value");

        // Test IotaResult methods
        store.put("test_key", "test_value".to_string());

        // Test async Result methods
        let async_value = store.get_async("test_key").await;
        assert_eq!(async_value, "async_value");

        // Test async IotaResult methods
        store.put_async("test_key", "test_value".to_string()).await;

        // Test original methods still work
        let try_value = store.try_get("test_key").unwrap();
        assert_eq!(try_value, "test_value");

        // Test original methods list still works
        let keys = store.list_keys();
        assert_eq!(keys, vec!["key1".to_string(), "key2".to_string()]);
    }
}

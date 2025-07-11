use std::{future::Future, pin::Pin};

use iota_proc_macros::{extend_impl_with_non_fallible, extend_trait_with_non_fallible};

// Define a BoxFuture type that matches what the macro expects
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// Mock IotaResult type for testing
type IotaResult<T> = Result<T, Box<dyn std::error::Error>>;

#[extend_trait_with_non_fallible]
pub trait ExtendedService {
    // Test with standard Result
    fn try_get(&self, key: &str) -> Result<String, String>;

    // Test with IotaResult
    fn try_put(&self, key: &str, value: String) -> IotaResult<()>;

    // Test with BoxFuture<Result<...>>
    fn try_get_async<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<String, String>>;

    // Test with BoxFuture<IotaResult<...>>
    fn try_put_async<'a>(&'a self, key: &'a str, value: String) -> BoxFuture<'a, IotaResult<()>>;

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

    fn try_get_async<'a>(&'a self, _key: &'a str) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async { Ok("async_value".to_string()) })
    }

    fn try_put_async<'a>(&'a self, _key: &'a str, _value: String) -> BoxFuture<'a, IotaResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn list_keys(&self) -> Vec<String> {
        vec!["key1".to_string(), "key2".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

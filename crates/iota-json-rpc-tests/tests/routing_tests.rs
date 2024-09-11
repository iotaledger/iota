// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::env;

use async_trait::async_trait;
use iota_json_rpc::{IotaRpcModule, JsonRpcServerBuilder, ServerType};
use iota_json_rpc_api::CLIENT_TARGET_API_VERSION_HEADER;
use iota_open_rpc::Module;
use iota_open_rpc_macros::open_rpc;
use jsonrpsee::{
    core::{client::ClientT, RpcResult},
    http_client::HttpClientBuilder,
    proc_macros::rpc,
    rpc_params, RpcModule,
};
use prometheus::Registry;

#[tokio::test]
async fn test_rpc_backward_compatibility() {
    let mut builder = JsonRpcServerBuilder::new("1.5", &Registry::new(), None, None);
    builder.register_module(TestApiModule).unwrap();

    let address = local_ip_utils::new_local_tcp_socket_for_testing();
    let _handle = builder
        .start(address, None, ServerType::Http, None)
        .await
        .unwrap();
    let url = format!("http://0.0.0.0:{}", address.port());

    // Test with un-versioned client
    let client = HttpClientBuilder::default().build(&url).unwrap();
    let response: String = client.request("test_foo", rpc_params!(true)).await.unwrap();
    assert_eq!("Some string", response);

    // try to access old method directly should fail
    let client = HttpClientBuilder::default().build(&url).unwrap();
    let response: RpcResult<String> = client.request("test_foo_1_5", rpc_params!("string")).await;
    assert!(response.is_err());

    // Test with versioned client, version > backward compatible method version
    let mut versioned_header = HeaderMap::new();
    versioned_header.insert(
        CLIENT_TARGET_API_VERSION_HEADER,
        HeaderValue::from_static("1.6"),
    );
    let client_with_new_header = HttpClientBuilder::default()
        .set_headers(versioned_header)
        .build(&url)
        .unwrap();

    let response: String = client_with_new_header
        .request("test_foo", rpc_params!(true))
        .await
        .unwrap();
    assert_eq!("Some string", response);

    // Test with versioned client, version = backward compatible method version
    let mut versioned_header = HeaderMap::new();
    versioned_header.insert(
        CLIENT_TARGET_API_VERSION_HEADER,
        HeaderValue::from_static("1.5"),
    );
    let client_with_new_header = HttpClientBuilder::default()
        .set_headers(versioned_header)
        .build(&url)
        .unwrap();

    let response: String = client_with_new_header
        .request(
            "test_foo",
            rpc_params!("old version expect string as input"),
        )
        .await
        .unwrap();
    assert_eq!("Some string from old method", response);

    // Test with versioned client, version < backward compatible method version
    let mut versioned_header = HeaderMap::new();
    versioned_header.insert(
        CLIENT_TARGET_API_VERSION_HEADER,
        HeaderValue::from_static("1.4"),
    );
    let client_with_new_header = HttpClientBuilder::default()
        .set_headers(versioned_header)
        .build(&url)
        .unwrap();

    let response: String = client_with_new_header
        .request(
            "test_foo",
            rpc_params!("old version expect string as input"),
        )
        .await
        .unwrap();
    assert_eq!("Some string from old method", response);
}

#[tokio::test]
async fn test_disable_routing() {
    env::set_var("DISABLE_BACKWARD_COMPATIBILITY", "true");

    let mut builder = JsonRpcServerBuilder::new("1.5", &Registry::new(), None, None);
    builder.register_module(TestApiModule).unwrap();

    let address = local_ip_utils::new_local_tcp_socket_for_testing();
    let _handle = builder
        .start(address, None, ServerType::Http, None)
        .await
        .unwrap();
    let url = format!("http://0.0.0.0:{}", address.port());

    // try to access old method directly should fail
    let client = HttpClientBuilder::default().build(&url).unwrap();
    let response: RpcResult<String> = client.request("test_foo_1_5", rpc_params!("string")).await;
    assert!(response.is_err());

    // Test with versioned client, version = backward compatible method version,
    // should fail because routing is disabled.
    let mut versioned_header = HeaderMap::new();
    versioned_header.insert(
        CLIENT_TARGET_API_VERSION_HEADER,
        HeaderValue::from_static("1.5"),
    );
    let client_with_new_header = HttpClientBuilder::default()
        .set_headers(versioned_header)
        .build(&url)
        .unwrap();

    let response: RpcResult<String> = client_with_new_header
        .request(
            "test_foo",
            rpc_params!("old version expect string as input"),
        )
        .await;
    assert!(response.is_err());
}

#[open_rpc(namespace = "test")]
#[rpc(server, client, namespace = "test")]
trait TestApi {
    #[method(name = "foo")]
    async fn foo(&self, some_bool: bool) -> RpcResult<String>;

    #[method(name = "foo", version <= "1.5")]
    async fn bar(&self, some_str: String) -> RpcResult<String>;
}

struct TestApiModule;

#[async_trait]
impl TestApiServer for TestApiModule {
    async fn foo(&self, _some_bool: bool) -> RpcResult<String> {
        Ok("Some string".into())
    }

    async fn bar(&self, _some_str: String) -> RpcResult<String> {
        Ok("Some string from old method".into())
    }
}

impl IotaRpcModule for TestApiModule {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }
    fn rpc_doc_module() -> Module {
        TestApiOpenRpc::module_doc()
    }
}

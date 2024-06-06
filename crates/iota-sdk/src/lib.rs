// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The Iota Rust SDK
//!
//! It aims at providing a similar SDK functionality like the one existing for
//! [TypeScript](https://github.com/iotaledger/iota/tree/main/sdk/typescript/).
//! Iota Rust SDK builds on top of the [JSON RPC API](https://docs.iota.io/iota-jsonrpc)
//! and therefore many of the return types are the ones specified in
//! [iota_types].
//!
//! The API is split in several parts corresponding to different functionalities
//! as following:
//! * [CoinReadApi] - provides read-only functions to work with the coins
//! * [EventApi] - provides event related functions functions to
//! * [GovernanceApi] - provides functionality related to staking
//! * [QuorumDriverApi] - provides functionality to execute a transaction
//! block and submit it to the fullnode(s)
//! * [ReadApi] - provides functions for retrieving data about different
//! objects and transactions
//! * <a href="../iota_transaction_builder/struct.TransactionBuilder.html"
//!   title="struct
//!   iota_transaction_builder::TransactionBuilder">TransactionBuilder</a> -
//!   provides functions for building transactions
//!
//! # Usage
//! The main way to interact with the API is through the [IotaClientBuilder],
//! which returns a [IotaClient] object from which the user can access the
//! various APIs.
//!
//! ## Getting Started
//! Add the Rust SDK to the project by running `cargo add iota-sdk` in the root
//! folder of your Rust project.
//!
//! The main building block for the Iota Rust SDK is the [IotaClientBuilder],
//! which provides a simple and straightforward way of connecting to a Iota
//! network and having access to the different available APIs.
//!
//! A simple example that connects to a running Iota local network,
//! the Iota devnet, and the Iota testnet is shown below.
//! To successfully run this program, make sure to spin up a local
//! network with a local validator, a fullnode, and a faucet server
//! (see [here](https://github.com/stefan-mysten/iota/tree/rust_sdk_api_examples/crates/iota-sdk/examples#preqrequisites) for more information).
//!
//! ```rust,no_run
//! use iota_sdk::IotaClientBuilder;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), anyhow::Error> {
//!     let iota = IotaClientBuilder::default()
//!         .build("http://127.0.0.1:9000") // provide the Iota network URL
//!         .await?;
//!     println!("Iota local network version: {:?}", iota.api_version());
//!
//!     // local Iota network, same result as above except using the dedicated function
//!     let iota_local = IotaClientBuilder::default().build_localnet().await?;
//!     println!("Iota local network version: {:?}", iota_local.api_version());
//!
//!     // Iota devnet running at `https://fullnode.devnet.io:443`
//!     let iota_devnet = IotaClientBuilder::default().build_devnet().await?;
//!     println!("Iota devnet version: {:?}", iota_devnet.api_version());
//!
//!     // Iota testnet running at `https://testnet.devnet.io:443`
//!     let iota_testnet = IotaClientBuilder::default().build_testnet().await?;
//!     println!("Iota testnet version: {:?}", iota_testnet.api_version());
//!     Ok(())
//! }
//! ```
//!
//! ## Examples
//!
//! For detailed examples, please check the APIs docs and the examples folder
//! in the [main repository](https://github.com/iotaledger/iota/tree/main/crates/iota-sdk/examples).

use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
pub use iota_json as json;
use iota_json_rpc_api::{
    CLIENT_SDK_TYPE_HEADER, CLIENT_SDK_VERSION_HEADER, CLIENT_TARGET_API_VERSION_HEADER,
};
pub use iota_json_rpc_types as rpc_types;
use iota_json_rpc_types::{
    IotaObjectDataFilter, IotaObjectDataOptions, IotaObjectResponse, IotaObjectResponseQuery,
    ObjectsPage,
};
use iota_transaction_builder::{DataReader, TransactionBuilder};
pub use iota_types as types;
use iota_types::base_types::{IotaAddress, ObjectID, ObjectInfo};
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HeaderMap, HeaderValue, HttpClient, HttpClientBuilder},
    rpc_params,
    ws_client::{WsClient, WsClientBuilder},
};
use move_core_types::language_storage::StructTag;
use serde_json::Value;

use crate::{
    apis::{CoinReadApi, EventApi, GovernanceApi, QuorumDriverApi, ReadApi},
    error::{Error, IotaRpcResult},
};

pub mod apis;
pub mod error;
pub mod iota_client_config;
pub mod json_rpc_error;
pub mod wallet_context;

pub const IOTA_COIN_TYPE: &str = "0x2::iota::IOTA";
pub const IOTA_LOCAL_NETWORK_URL: &str = "http://127.0.0.1:9000";
pub const IOTA_LOCAL_NETWORK_GAS_URL: &str = "http://127.0.0.1:5003/gas";
pub const IOTA_DEVNET_URL: &str = "https://fullnode.devnet.iota.io:443";
pub const IOTA_TESTNET_URL: &str = "https://fullnode.testnet.iota.io:443";

/// A Iota client builder for connecting to the Iota network
///
/// By default the `maximum concurrent requests` is set to 256 and
/// the `request timeout` is set to 60 seconds. These can be adjusted using the
/// `max_concurrent_requests` function, and the `request_timeout` function.
/// If you use the WebSocket, consider setting the `ws_ping_interval` field to a
/// value of your choice to prevent the inactive WS subscription being
/// disconnected due to proxy timeout.
///
/// # Examples
///
/// ```rust,no_run
/// use iota_sdk::IotaClientBuilder;
/// #[tokio::main]
/// async fn main() -> Result<(), anyhow::Error> {
///     let iota = IotaClientBuilder::default()
///         .build("http://127.0.0.1:9000")
///         .await?;
///
///     println!("Iota local network version: {:?}", iota.api_version());
///     Ok(())
/// }
/// ```
pub struct IotaClientBuilder {
    request_timeout: Duration,
    max_concurrent_requests: usize,
    ws_url: Option<String>,
    ws_ping_interval: Option<Duration>,
}

impl Default for IotaClientBuilder {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(60),
            max_concurrent_requests: 256,
            ws_url: None,
            ws_ping_interval: None,
        }
    }
}

impl IotaClientBuilder {
    /// Set the request timeout to the specified duration
    pub fn request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = request_timeout;
        self
    }

    /// Set the max concurrent requests allowed
    pub fn max_concurrent_requests(mut self, max_concurrent_requests: usize) -> Self {
        self.max_concurrent_requests = max_concurrent_requests;
        self
    }

    /// Set the WebSocket URL for the Iota network
    pub fn ws_url(mut self, url: impl AsRef<str>) -> Self {
        self.ws_url = Some(url.as_ref().to_string());
        self
    }

    /// Set the WebSocket ping interval
    pub fn ws_ping_interval(mut self, duration: Duration) -> Self {
        self.ws_ping_interval = Some(duration);
        self
    }

    /// Returns a [IotaClient] object connected to the Iota network running at
    /// the URI provided.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default()
    ///         .build("http://127.0.0.1:9000")
    ///         .await?;
    ///
    ///     println!("Iota local version: {:?}", iota.api_version());
    ///     Ok(())
    /// }
    /// ```
    pub async fn build(self, http: impl AsRef<str>) -> IotaRpcResult<IotaClient> {
        let client_version = env!("CARGO_PKG_VERSION");
        let mut headers = HeaderMap::new();
        headers.insert(
            CLIENT_TARGET_API_VERSION_HEADER,
            // in rust, the client version is the same as the target api version
            HeaderValue::from_static(client_version),
        );
        headers.insert(
            CLIENT_SDK_VERSION_HEADER,
            HeaderValue::from_static(client_version),
        );
        headers.insert(CLIENT_SDK_TYPE_HEADER, HeaderValue::from_static("rust"));

        let ws = if let Some(url) = self.ws_url {
            let mut builder = WsClientBuilder::default()
                .max_request_body_size(2 << 30)
                .max_concurrent_requests(self.max_concurrent_requests)
                .set_headers(headers.clone())
                .request_timeout(self.request_timeout);

            if let Some(duration) = self.ws_ping_interval {
                builder = builder.ping_interval(duration)
            }

            Some(builder.build(url).await?)
        } else {
            None
        };

        let http = HttpClientBuilder::default()
            .max_request_body_size(2 << 30)
            .max_concurrent_requests(self.max_concurrent_requests)
            .set_headers(headers.clone())
            .request_timeout(self.request_timeout)
            .build(http)?;

        let info = Self::get_server_info(&http, &ws).await?;

        let rpc = RpcClient { http, ws, info };
        let api = Arc::new(rpc);
        let read_api = Arc::new(ReadApi::new(api.clone()));
        let quorum_driver_api = QuorumDriverApi::new(api.clone());
        let event_api = EventApi::new(api.clone());
        let transaction_builder = TransactionBuilder::new(read_api.clone());
        let coin_read_api = CoinReadApi::new(api.clone());
        let governance_api = GovernanceApi::new(api.clone());

        Ok(IotaClient {
            api,
            transaction_builder,
            read_api,
            coin_read_api,
            event_api,
            quorum_driver_api,
            governance_api,
        })
    }

    /// Returns a [IotaClient] object that is ready to interact with the local
    /// development network (by default it expects the Iota network to be
    /// up and running at `127.0.0.1:9000`).
    ///
    /// For connecting to a custom URI, use the `build` function instead.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///
    ///     println!("Iota local version: {:?}", iota.api_version());
    ///     Ok(())
    /// }
    /// ```
    pub async fn build_localnet(self) -> IotaRpcResult<IotaClient> {
        self.build(IOTA_LOCAL_NETWORK_URL).await
    }

    /// Returns a [IotaClient] object that is ready to interact with the Iota
    /// devnet.
    ///
    /// For connecting to a custom URI, use the `build` function instead..
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_devnet().await?;
    ///
    ///     println!("{:?}", iota.api_version());
    ///     Ok(())
    /// }
    /// ```
    pub async fn build_devnet(self) -> IotaRpcResult<IotaClient> {
        self.build(IOTA_DEVNET_URL).await
    }

    /// Returns a [IotaClient] object that is ready to interact with the Iota
    /// testnet.
    ///
    /// For connecting to a custom URI, use the `build` function instead.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///
    ///     println!("{:?}", iota.api_version());
    ///     Ok(())
    /// }
    /// ```
    pub async fn build_testnet(self) -> IotaRpcResult<IotaClient> {
        self.build(IOTA_TESTNET_URL).await
    }

    /// Return the server information as a `ServerInfo` structure.
    ///
    /// Fails with an error if it cannot call the RPC discover.
    async fn get_server_info(
        http: &HttpClient,
        ws: &Option<WsClient>,
    ) -> Result<ServerInfo, Error> {
        let rpc_spec: Value = http.request("rpc.discover", rpc_params![]).await?;
        let version = rpc_spec
            .pointer("/info/version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::DataError("Fail parsing server version from rpc.discover endpoint.".into())
            })?;
        let rpc_methods = Self::parse_methods(&rpc_spec)?;

        let subscriptions = if let Some(ws) = ws {
            let rpc_spec: Value = ws.request("rpc.discover", rpc_params![]).await?;
            Self::parse_methods(&rpc_spec)?
        } else {
            Vec::new()
        };
        Ok(ServerInfo {
            rpc_methods,
            subscriptions,
            version: version.to_string(),
        })
    }

    fn parse_methods(server_spec: &Value) -> Result<Vec<String>, Error> {
        let methods = server_spec
            .pointer("/methods")
            .and_then(|methods| methods.as_array())
            .ok_or_else(|| {
                Error::DataError(
                    "Fail parsing server information from rpc.discover endpoint.".into(),
                )
            })?;

        Ok(methods
            .iter()
            .flat_map(|method| method["name"].as_str())
            .map(|s| s.into())
            .collect())
    }
}

/// IotaClient is the basic type that provides all the necessary abstractions
/// for interacting with the Iota network.
///
/// # Usage
///
/// Use [IotaClientBuilder] to build a [IotaClient].
///
/// # Examples
///
/// ```rust,no_run
/// use std::str::FromStr;
///
/// use iota_sdk::{types::base_types::IotaAddress, IotaClientBuilder};
///
/// #[tokio::main]
/// async fn main() -> Result<(), anyhow::Error> {
///     let iota = IotaClientBuilder::default()
///         .build("http://127.0.0.1:9000")
///         .await?;
///
///     println!("{:?}", iota.available_rpc_methods());
///     println!("{:?}", iota.available_subscriptions());
///     println!("{:?}", iota.api_version());
///
///     let address = IotaAddress::from_str("0x0000....0000")?;
///     let owned_objects = iota
///         .read_api()
///         .get_owned_objects(address, None, None, None)
///         .await?;
///
///     println!("{:?}", owned_objects);
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct IotaClient {
    api: Arc<RpcClient>,
    transaction_builder: TransactionBuilder,
    read_api: Arc<ReadApi>,
    coin_read_api: CoinReadApi,
    event_api: EventApi,
    quorum_driver_api: QuorumDriverApi,
    governance_api: GovernanceApi,
}

pub(crate) struct RpcClient {
    http: HttpClient,
    ws: Option<WsClient>,
    info: ServerInfo,
}

impl Debug for RpcClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RPC client. Http: {:?}, Websocket: {:?}",
            self.http, self.ws
        )
    }
}

/// ServerInfo contains all the useful information regarding the API version,
/// the available RPC calls, and subscriptions.
struct ServerInfo {
    rpc_methods: Vec<String>,
    subscriptions: Vec<String>,
    version: String,
}

impl IotaClient {
    /// Returns a list of RPC methods supported by the node the client is
    /// connected to.
    pub fn available_rpc_methods(&self) -> &Vec<String> {
        &self.api.info.rpc_methods
    }

    /// Returns a list of streaming/subscription APIs supported by the node the
    /// client is connected to.
    pub fn available_subscriptions(&self) -> &Vec<String> {
        &self.api.info.subscriptions
    }

    /// Returns the API version information as a string.
    ///
    /// The format of this string is `<major>.<minor>.<patch>`, e.g., `1.6.0`,
    /// and it is retrieved from the OpenRPC specification via the discover
    /// service method.
    pub fn api_version(&self) -> &str {
        &self.api.info.version
    }

    /// Verifies if the API version matches the server version and returns an
    /// error if they do not match.
    pub fn check_api_version(&self) -> IotaRpcResult<()> {
        let server_version = self.api_version();
        let client_version = env!("CARGO_PKG_VERSION");
        if server_version != client_version {
            return Err(Error::ServerVersionMismatch {
                client_version: client_version.to_string(),
                server_version: server_version.to_string(),
            });
        };
        Ok(())
    }

    /// Returns a reference to the coin read API.
    pub fn coin_read_api(&self) -> &CoinReadApi {
        &self.coin_read_api
    }

    /// Returns a reference to the event API.
    pub fn event_api(&self) -> &EventApi {
        &self.event_api
    }

    /// Returns a reference to the governance API.
    pub fn governance_api(&self) -> &GovernanceApi {
        &self.governance_api
    }

    /// Returns a reference to the quorum driver API.
    pub fn quorum_driver_api(&self) -> &QuorumDriverApi {
        &self.quorum_driver_api
    }

    /// Returns a reference to the read API.
    pub fn read_api(&self) -> &ReadApi {
        &self.read_api
    }

    /// Returns a reference to the transaction builder API.
    pub fn transaction_builder(&self) -> &TransactionBuilder {
        &self.transaction_builder
    }

    /// Returns a reference to the underlying http client.
    pub fn http(&self) -> &HttpClient {
        &self.api.http
    }

    /// Returns a reference to the underlying WebSocket client, if any.
    pub fn ws(&self) -> Option<&WsClient> {
        self.api.ws.as_ref()
    }
}

#[async_trait]
impl DataReader for ReadApi {
    async fn get_owned_objects(
        &self,
        address: IotaAddress,
        object_type: StructTag,
    ) -> Result<Vec<ObjectInfo>, anyhow::Error> {
        let mut result = vec![];
        let query = Some(IotaObjectResponseQuery {
            filter: Some(IotaObjectDataFilter::StructType(object_type)),
            options: Some(
                IotaObjectDataOptions::new()
                    .with_previous_transaction()
                    .with_type()
                    .with_owner(),
            ),
        });

        let mut has_next = true;
        let mut cursor = None;

        while has_next {
            let ObjectsPage {
                data,
                next_cursor,
                has_next_page,
            } = self
                .get_owned_objects(address, query.clone(), cursor, None)
                .await?;
            result.extend(
                data.iter()
                    .map(|r| r.clone().try_into())
                    .collect::<Result<Vec<_>, _>>()?,
            );
            cursor = next_cursor;
            has_next = has_next_page;
        }
        Ok(result)
    }

    async fn get_object_with_options(
        &self,
        object_id: ObjectID,
        options: IotaObjectDataOptions,
    ) -> Result<IotaObjectResponse, anyhow::Error> {
        Ok(self.get_object_with_options(object_id, options).await?)
    }

    /// Returns the reference gas price as a u64 or an error otherwise
    async fn get_reference_gas_price(&self) -> Result<u64, anyhow::Error> {
        Ok(self.get_reference_gas_price().await?)
    }
}

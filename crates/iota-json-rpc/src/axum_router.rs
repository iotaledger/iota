// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Json, State},
};
use hyper::HeaderMap;
use iota_json_rpc_api::CLIENT_TARGET_API_VERSION_HEADER;
use jsonrpsee::{
    core::server::{helpers::MethodSink, Methods},
    server::RandomIntegerIdProvider,
    types::{
        error::{ErrorCode, BATCHES_NOT_SUPPORTED_CODE, BATCHES_NOT_SUPPORTED_MSG},
        ErrorObject, Id, InvalidRequest, Params, Request,
    },
    BoundedSubscriptions, ConnectionId, Extensions, MethodCallback, MethodKind, MethodResponse,
};
use serde_json::value::RawValue;

use crate::{
    logger::{Logger, TransportProtocol},
    routing_layer::RpcRouter,
};

pub const MAX_RESPONSE_SIZE: u32 = 2 << 30;

#[derive(Clone, Debug)]
pub struct JsonRpcService<L> {
    logger: L,

    id_provider: Arc<RandomIntegerIdProvider>,

    /// Registered server methods.
    methods: Methods,
    extensions: Extensions,
    rpc_router: RpcRouter,
}

impl<L> JsonRpcService<L> {
    pub fn new(methods: Methods, rpc_router: RpcRouter, logger: L, extensions: Extensions) -> Self {
        Self {
            methods,
            rpc_router,
            logger,
            extensions,
            id_provider: Arc::new(RandomIntegerIdProvider),
        }
    }
}

impl<L: Logger> JsonRpcService<L> {
    fn call_data(&self) -> CallData<'_, L> {
        CallData {
            logger: &self.logger,
            methods: &self.methods,
            rpc_router: &self.rpc_router,
            extensions: &self.extensions,
            max_response_body_size: MAX_RESPONSE_SIZE,
            request_start: self.logger.on_request(TransportProtocol::Http),
        }
    }

    fn ws_call_data<'c, 'a: 'c, 'b: 'c>(
        &'a self,
        bounded_subscriptions: BoundedSubscriptions,
        sink: &'b MethodSink,
    ) -> ws::WsCallData<'c, L> {
        ws::WsCallData {
            logger: &self.logger,
            methods: &self.methods,
            extensions: &self.extensions,
            max_response_body_size: MAX_RESPONSE_SIZE,
            request_start: self.logger.on_request(TransportProtocol::Http),
            bounded_subscriptions,
            id_provider: &*self.id_provider,
            sink,
        }
    }
}

/// Create a response body.
fn from_template<S: Into<Body>>(
    status: hyper::StatusCode,
    body: S,
    content_type: &'static str,
) -> hyper::Response<Body> {
    hyper::Response::builder()
        .status(status)
        .header(
            "content-type",
            hyper::header::HeaderValue::from_static(content_type),
        )
        .body(body.into())
        // Parsing `StatusCode` and `HeaderValue` is infalliable but
        // parsing body content is not.
        .expect("Unable to parse response body for type conversion")
}

/// Create a valid JSON response.
pub(crate) fn ok_response(body: String) -> hyper::Response<Body> {
    const JSON: &str = "application/json; charset=utf-8";
    from_template(hyper::StatusCode::OK, body, JSON)
}

pub async fn json_rpc_handler<L: Logger>(
    State(service): State<JsonRpcService<L>>,
    headers: HeaderMap,
    Json(raw_request): Json<Box<RawValue>>,
) -> impl axum::response::IntoResponse {
    // Get version from header.
    let api_version = headers
        .get(CLIENT_TARGET_API_VERSION_HEADER)
        .and_then(|h| h.to_str().ok());
    let response = process_raw_request(&service, api_version, raw_request.get()).await;

    ok_response(response.into_result())
}

async fn process_raw_request<L: Logger>(
    service: &JsonRpcService<L>,
    api_version: Option<&str>,
    raw_request: &str,
) -> MethodResponse {
    if let Ok(request) = serde_json::from_str::<Request>(raw_request) {
        process_request(request, api_version, service.call_data()).await
    } else if let Ok(_batch) = serde_json::from_str::<Vec<&RawValue>>(raw_request) {
        MethodResponse::error(
            Id::Null,
            ErrorObject::borrowed(BATCHES_NOT_SUPPORTED_CODE, BATCHES_NOT_SUPPORTED_MSG, None),
        )
    } else {
        let (id, code) = prepare_error(raw_request);
        MethodResponse::error(id, ErrorObject::from(code))
    }
}

async fn process_request<L: Logger>(
    req: Request<'_>,
    api_version: Option<&str>,
    call: CallData<'_, L>,
) -> MethodResponse {
    let CallData {
        methods,
        rpc_router,
        logger,
        extensions,
        max_response_body_size,
        request_start,
    } = call;
    let conn_id = ConnectionId(0); // unused

    let params = Params::new(req.params.as_ref().map(|params| params.get()));
    let name = rpc_router.route(&req.method, api_version);
    let id = req.id;

    let response = match methods.method_with_name(name) {
        None => {
            logger.on_call(
                name,
                params.clone(),
                MethodKind::NotFound,
                TransportProtocol::Http,
            );
            MethodResponse::error(id, ErrorObject::from(ErrorCode::MethodNotFound))
        }
        Some((name, method)) => match method {
            MethodCallback::Sync(callback) => {
                logger.on_call(
                    name,
                    params.clone(),
                    MethodKind::MethodCall,
                    TransportProtocol::Http,
                );
                (callback)(
                    id,
                    params,
                    max_response_body_size as usize,
                    extensions.clone(),
                )
            }
            MethodCallback::Async(callback) => {
                logger.on_call(
                    name,
                    params.clone(),
                    MethodKind::MethodCall,
                    TransportProtocol::Http,
                );

                let id = id.into_owned();
                let params = params.into_owned();

                (callback)(
                    id,
                    params,
                    conn_id,
                    max_response_body_size as usize,
                    extensions.clone(),
                )
                .await
            }
            MethodCallback::Subscription(_) | MethodCallback::Unsubscription(_) => {
                logger.on_call(
                    name,
                    params.clone(),
                    MethodKind::NotFound,
                    TransportProtocol::Http,
                );
                // Subscriptions not supported on HTTP
                MethodResponse::error(id, ErrorObject::from(ErrorCode::InternalError))
            }
        },
    };

    logger.on_result(
        name,
        response.is_success(),
        response.as_error_code(),
        request_start,
        TransportProtocol::Http,
    );
    response
}

/// Figure out if this is a sufficiently complete request that we can extract an
/// [`Id`] out of, or just plain unparsable garbage.
pub fn prepare_error(data: &str) -> (Id<'_>, ErrorCode) {
    match serde_json::from_str::<InvalidRequest>(data) {
        Ok(InvalidRequest { id }) => (id, ErrorCode::InvalidRequest),
        Err(_) => (Id::Null, ErrorCode::ParseError),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CallData<'a, L: Logger> {
    logger: &'a L,
    methods: &'a Methods,
    rpc_router: &'a RpcRouter,
    extensions: &'a Extensions,
    max_response_body_size: u32,
    request_start: L::Instant,
}

pub mod ws {
    use axum::{
        extract::{
            ws::{Message, WebSocket},
            WebSocketUpgrade,
        },
        response::Response,
    };
    use jsonrpsee::{
        core::server::helpers::MethodSink, server::IdProvider,
        types::error::reject_too_many_subscriptions, SubscriptionState,
    };
    use tokio::sync::mpsc;

    use super::*;

    const MAX_WS_MESSAGE_BUFFER: usize = 100;

    #[derive(Debug, Clone)]
    pub(crate) struct WsCallData<'a, L: Logger> {
        pub bounded_subscriptions: BoundedSubscriptions,
        pub id_provider: &'a dyn IdProvider,
        pub methods: &'a Methods,
        pub extensions: &'a Extensions,
        pub max_response_body_size: u32,
        pub sink: &'a MethodSink,
        pub logger: &'a L,
        pub request_start: L::Instant,
    }

    // A WebSocket handler that echos any message it receives.
    //
    // This one we'll be integration testing so it can be written in the regular
    // way.
    pub async fn ws_json_rpc_upgrade<L: Logger>(
        ws: WebSocketUpgrade,
        State(service): State<JsonRpcService<L>>,
    ) -> Response {
        ws.on_upgrade(|ws| ws_json_rpc_handler(ws, service))
    }

    async fn ws_json_rpc_handler<L: Logger>(mut socket: WebSocket, service: JsonRpcService<L>) {
        #[allow(clippy::disallowed_methods)]
        let (tx, mut rx) = mpsc::channel::<String>(MAX_WS_MESSAGE_BUFFER);
        let sink = MethodSink::new_with_limit(tx, MAX_RESPONSE_SIZE);
        let bounded_subscriptions = BoundedSubscriptions::new(100);

        loop {
            tokio::select! {
                maybe_message = socket.recv() => {
                    if let Some(Ok(message)) = maybe_message {
                        if let Message::Text(msg) = message {
                            let response =
                                process_raw_request(&service, &msg, bounded_subscriptions.clone(), &sink).await;
                            if let Some(response) = response {
                                sink.send(response.into_result()).await.ok();
                            }
                        }
                    } else {
                        break;
                    }
                },
                Some(response) = rx.recv() => {
                    if socket.send(Message::Text(response)).await.is_err() {
                        break;
                    }
                },
            }
        }
    }

    async fn process_raw_request<L: Logger>(
        service: &JsonRpcService<L>,
        raw_request: &str,
        bounded_subscriptions: BoundedSubscriptions,
        sink: &MethodSink,
    ) -> Option<MethodResponse> {
        if let Ok(request) = serde_json::from_str::<Request>(raw_request) {
            process_request(request, service.ws_call_data(bounded_subscriptions, sink)).await
        } else if let Ok(_batch) = serde_json::from_str::<Vec<&RawValue>>(raw_request) {
            Some(MethodResponse::error(
                Id::Null,
                ErrorObject::borrowed(BATCHES_NOT_SUPPORTED_CODE, BATCHES_NOT_SUPPORTED_MSG, None),
            ))
        } else {
            let (id, code) = prepare_error(raw_request);
            Some(MethodResponse::error(id, ErrorObject::from(code)))
        }
    }

    async fn process_request<L: Logger>(
        req: Request<'_>,
        call: WsCallData<'_, L>,
    ) -> Option<MethodResponse> {
        let WsCallData {
            methods,
            logger,
            extensions,
            max_response_body_size,
            request_start,
            bounded_subscriptions,
            id_provider,
            sink,
        } = call;
        let conn_id = ConnectionId(0); // unused

        let params = Params::new(req.params.as_ref().map(|params| params.get()));
        let name = &req.method;
        let id = req.id;

        let response = match methods.method_with_name(name) {
            None => {
                logger.on_call(
                    name,
                    params.clone(),
                    MethodKind::NotFound,
                    TransportProtocol::Http,
                );
                Some(MethodResponse::error(
                    id,
                    ErrorObject::from(ErrorCode::MethodNotFound),
                ))
            }
            Some((name, method)) => match method {
                MethodCallback::Sync(callback) => {
                    logger.on_call(
                        name,
                        params.clone(),
                        MethodKind::MethodCall,
                        TransportProtocol::Http,
                    );
                    tracing::info!("calling {name} sync");
                    Some((callback)(
                        id,
                        params,
                        max_response_body_size as usize,
                        extensions.clone(),
                    ))
                }
                MethodCallback::Async(callback) => {
                    logger.on_call(
                        name,
                        params.clone(),
                        MethodKind::MethodCall,
                        TransportProtocol::Http,
                    );

                    let id = id.into_owned();
                    let params = params.into_owned();

                    tracing::info!("calling {name} async");
                    Some(
                        (callback)(
                            id,
                            params,
                            conn_id,
                            max_response_body_size as usize,
                            extensions.clone(),
                        )
                        .await,
                    )
                }

                MethodCallback::Subscription(callback) => {
                    logger.on_call(
                        name,
                        params.clone(),
                        MethodKind::Subscription,
                        TransportProtocol::WebSocket,
                    );
                    if let Some(subscription_permit) = bounded_subscriptions.acquire() {
                        let conn_state = SubscriptionState {
                            conn_id,
                            subscription_permit,
                            id_provider,
                        };
                        (callback)(
                            id.clone(),
                            params,
                            sink.clone(),
                            conn_state,
                            extensions.clone(),
                        )
                        .await;
                        None
                    } else {
                        Some(MethodResponse::error(
                            id,
                            reject_too_many_subscriptions(bounded_subscriptions.max()),
                        ))
                    }
                }

                MethodCallback::Unsubscription(callback) => {
                    logger.on_call(
                        name,
                        params.clone(),
                        MethodKind::Unsubscription,
                        TransportProtocol::WebSocket,
                    );

                    Some(callback(
                        id,
                        params,
                        conn_id,
                        max_response_body_size as usize,
                        extensions.clone(),
                    ))
                }
            },
        };

        if let Some(response) = &response {
            logger.on_result(
                name,
                response.is_success(),
                response.as_error_code(),
                request_start,
                TransportProtocol::WebSocket,
            );
        }
        response
    }
}

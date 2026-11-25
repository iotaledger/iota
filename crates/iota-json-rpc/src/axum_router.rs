// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use axum::{
    extract::{ConnectInfo, FromRequest, Json, Request as HttpRequest, State},
    response::{IntoResponse, Response},
};
use hyper::HeaderMap;
use iota_json_rpc_api::{
    CLIENT_TARGET_API_VERSION_HEADER, TRANSACTION_EXECUTION_CLIENT_ERROR_CODE,
    TRANSACTION_NOT_FOUND_ERROR_CODE,
};
use iota_traffic_controller::{
    ClientIpStatus, TrafficController, get_client_ip, policies::TrafficTally,
};
use iota_types::traffic_control::{ClientIdSource, PolicyConfig, Weight};
use jsonrpsee::{
    ConnectionId, Extensions, MethodCallback, MethodKind, MethodResponse,
    core::server::Methods,
    types::{
        ErrorObject, Id, InvalidRequest, Params, Request,
        error::{BATCHES_NOT_SUPPORTED_CODE, BATCHES_NOT_SUPPORTED_MSG, ErrorCode},
    },
};
use serde_json::value::RawValue;
use tracing::error;

use crate::{logger::Logger, routing_layer::RpcRouter};

pub const MAX_RESPONSE_SIZE: u32 = 2 << 30;
const TOO_MANY_REQUESTS_MSG: &str = "Too many requests";

#[derive(Clone, Debug)]
pub struct JsonRpcService<L> {
    logger: L,

    /// Registered server methods.
    methods: Methods,
    extensions: Extensions,
    rpc_router: RpcRouter,
    traffic_controller: Option<Arc<TrafficController>>,
    client_id_source: Option<ClientIdSource>,
}

impl<L> JsonRpcService<L> {
    pub fn new(
        methods: Methods,
        rpc_router: RpcRouter,
        logger: L,
        traffic_controller: Option<Arc<TrafficController>>,
        policy_config: Option<PolicyConfig>,
        extensions: Extensions,
    ) -> Self {
        Self {
            methods,
            rpc_router,
            logger,
            extensions,
            traffic_controller,
            client_id_source: policy_config.map(|policy| policy.client_id_source),
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
            request_start: self.logger.on_request(),
        }
    }
}

/// Create a response body.
fn from_template<S: Into<axum::body::Body>>(
    status: hyper::StatusCode,
    body: S,
    content_type: &'static str,
) -> Response {
    Response::builder()
        .status(status)
        .header(
            "content-type",
            hyper::header::HeaderValue::from_static(content_type),
        )
        .body(body.into())
        // Parsing `StatusCode` and `HeaderValue` is infallible but
        // parsing body content is not.
        .expect("unable to parse response body for type conversion")
}

/// Create a valid JSON response.
pub(crate) fn ok_response(body: String) -> Response {
    const JSON: &str = "application/json; charset=utf-8";
    from_template(hyper::StatusCode::OK, body, JSON)
}

pub async fn json_rpc_handler<L: Logger>(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    State(service): State<JsonRpcService<L>>,
    request: HttpRequest,
) -> Response {
    let headers = request.headers().clone();
    let client = match client_ip(&service, &headers, client_addr) {
        Ok(client) => client,
        Err(refused_response) => return ok_response(refused_response.into_result()),
    };
    // Check whether the client may be served before reading the body, so a
    // refused client's body is neither read nor parsed.
    if let Some(traffic_controller) = &service.traffic_controller {
        if let Err(blocked_response) = handle_traffic_req(traffic_controller.clone(), &client) {
            return ok_response(blocked_response.into_result());
        }
    }
    let raw_request = match Json::<Box<RawValue>>::from_request(request, &()).await {
        Ok(Json(raw_request)) => raw_request,
        Err(rejection) => {
            // Any body the `Json` extractor rejects (not JSON, no JSON content
            // type, too large, or not fully received) is tallied like a request
            // the node cannot parse.
            if let Some(traffic_controller) = &service.traffic_controller {
                let parse_error =
                    MethodResponse::error(Id::Null, ErrorObject::from(ErrorCode::ParseError));
                handle_traffic_resp(traffic_controller.clone(), client, &parse_error);
            }
            return rejection.into_response();
        }
    };
    // Get version from header.
    let api_version = headers
        .get(CLIENT_TARGET_API_VERSION_HEADER)
        .and_then(|h| h.to_str().ok());
    let response = process_raw_request(&service, api_version, raw_request.get(), client).await;

    ok_response(response.into_result())
}

/// Resolves the client IP used for traffic control. Returns the response to
/// send instead of serving the request when the request must be refused.
fn client_ip<L>(
    service: &JsonRpcService<L>,
    headers: &HeaderMap,
    client_addr: SocketAddr,
) -> Result<Option<IpAddr>, MethodResponse> {
    let Some(source) = service.client_id_source.as_ref() else {
        return Ok(None);
    };
    match get_client_ip(headers, Some(client_addr), source) {
        ClientIpStatus::Ok(client) => Ok(Some(client)),
        status => {
            error!("{status}");
            if status.should_refuse_request() {
                let err_obj = ErrorObject::owned(
                    ErrorCode::InvalidRequest.code(),
                    status.to_string(),
                    None::<()>,
                );
                return Err(MethodResponse::error(Id::Null, err_obj));
            }
            Ok(None)
        }
    }
}

async fn process_raw_request<L: Logger>(
    service: &JsonRpcService<L>,
    api_version: Option<&str>,
    raw_request: &str,
    client: Option<IpAddr>,
) -> MethodResponse {
    let response = if let Ok(request) = serde_json::from_str::<Request>(raw_request) {
        process_request(request, api_version, service.call_data()).await
    } else if let Ok(_batch) = serde_json::from_str::<Vec<&RawValue>>(raw_request) {
        MethodResponse::error(
            Id::Null,
            ErrorObject::borrowed(BATCHES_NOT_SUPPORTED_CODE, BATCHES_NOT_SUPPORTED_MSG, None),
        )
    } else {
        let (id, code) = prepare_error(raw_request);
        MethodResponse::error(id, ErrorObject::from(code))
    };
    // A request the node cannot parse is tallied like any other request, so
    // that a client cannot send malformed requests without a limit.
    if let Some(traffic_controller) = &service.traffic_controller {
        handle_traffic_resp(traffic_controller.clone(), client, &response);
    }
    response
}

fn handle_traffic_req(
    traffic_controller: Arc<TrafficController>,
    client: &Option<IpAddr>,
) -> Result<(), MethodResponse> {
    if !traffic_controller.check(client, &None) {
        // Entity in blocklist
        let err_obj =
            ErrorObject::borrowed(ErrorCode::ServerIsBusy.code(), TOO_MANY_REQUESTS_MSG, None);
        Err(MethodResponse::error(Id::Null, err_obj))
    } else {
        Ok(())
    }
}

fn handle_traffic_resp(
    traffic_controller: Arc<TrafficController>,
    client: Option<IpAddr>,
    response: &MethodResponse,
) {
    let error = response.as_error_code().map(ErrorCode::from);
    traffic_controller.tally(TrafficTally {
        direct: client,
        through_fullnode: None,
        error_info: error.map(|e| {
            let error_type = e.to_string();
            let error_weight = normalize(e);
            (error_weight, error_type)
        }),
        // For now, count everything as spam with equal weight
        // on the rpc node side, including gas-charging endpoints
        // such as `iota_executeTransactionBlock`, as this can enable
        // node operators who wish to rate limit their transaction
        // traffic and incentivize high volume clients to choose a
        // suitable rpc provider (or run their own). Later we may want
        // to provide a weight distribution based on the method being called.
        spam_weight: Weight::one(),
    });
}

// TODO: refine error matching here
fn normalize(err: ErrorCode) -> Weight {
    match err {
        ErrorCode::ParseError | ErrorCode::InvalidRequest | ErrorCode::InvalidParams => {
            Weight::one()
        }
        ErrorCode::ServerError(i) if i == TRANSACTION_NOT_FOUND_ERROR_CODE => Weight::zero(),
        // e.g. invalid client signature
        ErrorCode::ServerError(i) if i == TRANSACTION_EXECUTION_CLIENT_ERROR_CODE => Weight::one(),
        _ => Weight::zero(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
        sync::Arc,
    };

    use axum::{body::Body, extract::ConnectInfo, http::Request, routing::post};
    use hyper::StatusCode;
    use iota_json_rpc_api::{
        TRANSACTION_EXECUTION_CLIENT_ERROR_CODE, TRANSACTION_NOT_FOUND_ERROR_CODE,
    };
    use iota_traffic_controller::TrafficController;
    use iota_types::traffic_control::{ClientIdSource, PolicyConfig, PolicyType, Weight};
    use jsonrpsee::{Extensions, core::server::Methods, types::ErrorCode};
    use tower::ServiceExt;

    use super::{JsonRpcService, TOO_MANY_REQUESTS_MSG, json_rpc_handler, normalize};
    use crate::routing_layer::RpcRouter;

    const CLIENT: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9000);

    /// A JSON-RPC router whose traffic controller runs the given spam and error
    /// policies.
    fn router(
        spam_policy_type: PolicyType,
        error_policy_type: PolicyType,
    ) -> (axum::Router, Arc<TrafficController>) {
        let policy_config = PolicyConfig {
            client_id_source: ClientIdSource::SocketAddr,
            connection_blocklist_ttl_sec: 120,
            spam_policy_type,
            error_policy_type,
            spam_sample_rate: Weight::one(),
            dry_run: false,
            ..Default::default()
        };
        let traffic_controller = Arc::new(TrafficController::init_for_test(policy_config.clone()));
        let service = JsonRpcService::new(
            Methods::new(),
            RpcRouter::new(HashMap::new(), false),
            (),
            Some(traffic_controller.clone()),
            Some(policy_config),
            Extensions::new(),
        );
        let router = axum::Router::new()
            .route("/", post(json_rpc_handler::<()>))
            .with_state(service);
        (router, traffic_controller)
    }

    async fn send(
        router: &axum::Router,
        content_type: Option<&str>,
        body: impl Into<Body>,
    ) -> (StatusCode, String) {
        let mut request = Request::post("/").extension(ConnectInfo(CLIENT));
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        let response = router
            .clone()
            .oneshot(request.body(body.into()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    /// A request with a JSON body still reaches the method dispatch and is
    /// tallied.
    #[tokio::test]
    async fn parsed_requests_are_served_and_tallied() {
        let (router, traffic_controller) = router(PolicyType::TestNConnIP(1), PolicyType::NoOp);
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"unknown_method"}"#;

        let (status, body) = send(&router, Some("application/json"), request).await;

        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("Method not found"), "{body}");
        assert!(!traffic_controller.check(&Some(CLIENT.ip()), &None));
    }

    /// A body the `Json` extractor rejects keeps its status and is tallied, and
    /// the same request is refused once the client is blocked.
    #[tokio::test]
    async fn rejected_bodies_are_tallied_and_checked() {
        let json = Some("application/json");
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"rpc.discover"}"#;
        let too_large = vec![b' '; 3 * 1024 * 1024];
        for (content_type, body, status) in [
            (json, b"not-json".to_vec(), StatusCode::BAD_REQUEST),
            (
                None,
                request.as_bytes().to_vec(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
            (json, too_large, StatusCode::PAYLOAD_TOO_LARGE),
        ] {
            let (router, traffic_controller) = router(PolicyType::TestNConnIP(1), PolicyType::NoOp);

            assert_eq!(send(&router, content_type, body.clone()).await.0, status);
            assert!(
                !traffic_controller.check(&Some(CLIENT.ip()), &None),
                "{status}: the rejected request must be tallied"
            );

            let (blocked_status, blocked_body) = send(&router, content_type, body).await;
            assert_eq!(blocked_status, StatusCode::OK, "{status}");
            assert!(
                blocked_body.contains(TOO_MANY_REQUESTS_MSG),
                "{blocked_body}"
            );
        }
    }

    /// A body the `Json` extractor rejects is tallied as a client error.
    #[tokio::test]
    async fn rejected_bodies_feed_the_error_policy() {
        let (router, traffic_controller) = router(PolicyType::NoOp, PolicyType::TestNConnIP(1));

        let (status, _) = send(&router, Some("application/json"), "not-json").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(!traffic_controller.check(&Some(CLIENT.ip()), &None));
    }

    #[test]
    fn transaction_not_found_has_zero_error_weight() {
        assert_eq!(
            normalize(ErrorCode::ServerError(TRANSACTION_NOT_FOUND_ERROR_CODE)),
            Weight::zero()
        );
    }

    #[test]
    fn client_errors_have_full_error_weight() {
        assert_eq!(normalize(ErrorCode::ParseError), Weight::one());
        assert_eq!(normalize(ErrorCode::InvalidParams), Weight::one());
        assert_eq!(
            normalize(ErrorCode::ServerError(
                TRANSACTION_EXECUTION_CLIENT_ERROR_CODE
            )),
            Weight::one()
        );
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

    let name = rpc_router.route(&req.method, api_version);
    let params = Params::new(req.params.as_ref().map(|params| params.get()));

    let id = req.id;

    let response = match methods.method_with_name(name) {
        None => {
            logger.on_call(name, params.clone(), MethodKind::NotFound);
            MethodResponse::error(id, ErrorObject::from(ErrorCode::MethodNotFound))
        }
        Some((name, method)) => match method {
            MethodCallback::Sync(callback) => {
                logger.on_call(name, params.clone(), MethodKind::MethodCall);
                (callback)(
                    id,
                    params,
                    max_response_body_size as usize,
                    extensions.clone(),
                )
            }
            MethodCallback::Async(callback) => {
                logger.on_call(name, params.clone(), MethodKind::MethodCall);

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
                logger.on_call(name, params.clone(), MethodKind::NotFound);
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

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
    ClientAccess, ClientIpStatus, SimulationGasRefusal, SimulationGasReservation,
    TrafficController, get_client_ip, policies::TrafficTally,
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

use crate::{error::Error, logger::Logger, routing_layer::RpcRouter};

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

/// Request extension through which a simulation method reserves gas from its
/// client's simulation gas balance. It carries the client of the request and
/// what the traffic controller decided for it.
#[derive(Clone)]
struct SimulationGasHandle {
    traffic_controller: Arc<TrafficController>,
    client: Option<IpAddr>,
    access: ClientAccess,
}

impl SimulationGasHandle {
    /// Reserves gas for one simulation, as
    /// [`TrafficController::reserve_simulation_gas`] does.
    fn reserve(&self) -> Result<Option<SimulationGasReservation>, SimulationGasRefusal> {
        self.traffic_controller
            .reserve_simulation_gas(self.client, self.access)
    }
}

/// Reserves gas for one simulation of the request's client. Refused while the
/// client is over its simulation gas limit. `None` when the request has no gas
/// limit: traffic control is off, no simulation gas limit is set, an allow
/// rule serves the client, or the extensions carry no handle.
pub(crate) fn reserve_simulation_gas(
    extensions: &Extensions,
) -> Result<Option<SimulationGasReservation>, Error> {
    let Some(handle) = extensions.get::<SimulationGasHandle>() else {
        return Ok(None);
    };
    handle
        .reserve()
        .map_err(|refusal| Error::ServerIsBusy(refusal.to_string()))
}

impl<L: Logger> JsonRpcService<L> {
    /// The call data of one request. Its extensions are the service's, with a
    /// [`SimulationGasHandle`] for the request added when traffic control is
    /// on.
    fn call_data(&self, client: Option<IpAddr>, access: ClientAccess) -> CallData<'_, L> {
        let mut extensions = self.extensions.clone();
        if let Some(traffic_controller) = &self.traffic_controller {
            extensions.insert(SimulationGasHandle {
                traffic_controller: traffic_controller.clone(),
                client,
                access,
            });
        }
        CallData {
            logger: &self.logger,
            methods: &self.methods,
            rpc_router: &self.rpc_router,
            extensions,
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
    let access = match &service.traffic_controller {
        Some(traffic_controller) => match handle_traffic_req(traffic_controller, &client) {
            Ok(access) => access,
            Err(blocked_response) => return ok_response(blocked_response.into_result()),
        },
        None => ClientAccess::Limited,
    };
    let raw_request = match Json::<Box<RawValue>>::from_request(request, &()).await {
        Ok(Json(raw_request)) => raw_request,
        Err(rejection) => {
            // Any body the `Json` extractor rejects (not JSON, no JSON content
            // type, too large, or not fully received) is tallied like a request
            // the node cannot parse.
            if let Some(traffic_controller) = &service.traffic_controller {
                let parse_error =
                    MethodResponse::error(Id::Null, ErrorObject::from(ErrorCode::ParseError));
                handle_traffic_resp(traffic_controller, client, access, &parse_error);
            }
            return rejection.into_response();
        }
    };
    // Get version from header.
    let api_version = headers
        .get(CLIENT_TARGET_API_VERSION_HEADER)
        .and_then(|h| h.to_str().ok());
    let response =
        process_raw_request(&service, api_version, raw_request.get(), client, access).await;

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
    access: ClientAccess,
) -> MethodResponse {
    let response = if let Ok(request) = serde_json::from_str::<Request>(raw_request) {
        process_request(request, api_version, service.call_data(client, access)).await
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
        handle_traffic_resp(traffic_controller, client, access, &response);
    }
    response
}

/// What the traffic controller decided for a request from `client`, or the
/// response to send when it refuses the request.
fn handle_traffic_req(
    traffic_controller: &TrafficController,
    client: &Option<IpAddr>,
) -> Result<ClientAccess, MethodResponse> {
    let access = traffic_controller.check(client, &None);
    if access.is_served() {
        Ok(access)
    } else {
        let err_obj =
            ErrorObject::borrowed(ErrorCode::ServerIsBusy.code(), TOO_MANY_REQUESTS_MSG, None);
        Err(MethodResponse::error(Id::Null, err_obj))
    }
}

/// Tallies the request, unless an allow rule serves its client.
fn handle_traffic_resp(
    traffic_controller: &TrafficController,
    client: Option<IpAddr>,
    access: ClientAccess,
    response: &MethodResponse,
) {
    if access == ClientAccess::Allowed {
        return;
    }
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
    use iota_traffic_controller::{ClientAccess, TrafficController};
    use iota_types::traffic_control::{
        AccessRuleList, AccessRules, ClientIdSource, PolicyConfig, PolicyType,
        SimulationGasLimitConfig, Weight,
    };
    use jsonrpsee::{Extensions, core::server::Methods, types::ErrorCode};
    use tower::ServiceExt;

    use super::{
        JsonRpcService, TOO_MANY_REQUESTS_MSG, json_rpc_handler, normalize, reserve_simulation_gas,
    };
    use crate::routing_layer::RpcRouter;

    const CLIENT: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9000);

    /// A JSON-RPC router whose traffic controller runs the given spam and error
    /// policies.
    fn router(
        spam_policy_type: PolicyType,
        error_policy_type: PolicyType,
    ) -> (axum::Router, Arc<TrafficController>) {
        router_with_rules(spam_policy_type, error_policy_type, AccessRules::default())
    }

    /// Like [`router`], with the given access rules.
    fn router_with_rules(
        spam_policy_type: PolicyType,
        error_policy_type: PolicyType,
        access_rules: AccessRules,
    ) -> (axum::Router, Arc<TrafficController>) {
        router_with_policy_config_and_methods(
            PolicyConfig {
                spam_policy_type,
                error_policy_type,
                access_rules,
                ..Default::default()
            },
            Methods::new(),
        )
    }

    /// A JSON-RPC router serving `methods`, whose traffic controller runs
    /// `policy_config` without dry run, reading the client from the socket.
    fn router_with_policy_config_and_methods(
        policy_config: PolicyConfig,
        methods: Methods,
    ) -> (axum::Router, Arc<TrafficController>) {
        let policy_config = PolicyConfig {
            client_id_source: ClientIdSource::SocketAddr,
            connection_blocklist_ttl_sec: 120,
            spam_sample_rate: Weight::one(),
            dry_run: false,
            ..policy_config
        };
        let traffic_controller = Arc::new(TrafficController::init_for_test(policy_config.clone()));
        traffic_controller.set_max_simulation_computation(100);
        let service = JsonRpcService::new(
            methods,
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
        assert!(
            !traffic_controller
                .check(&Some(CLIENT.ip()), &None)
                .is_served()
        );
    }

    /// A request from a client that an allow rule serves is not tallied, so no
    /// policy blocks the client.
    #[tokio::test]
    async fn requests_from_an_allowed_client_are_not_tallied() {
        let (router, traffic_controller) = router_with_rules(
            PolicyType::TestNConnIP(1),
            PolicyType::TestNConnIP(1),
            AccessRules {
                allow: vec![CLIENT.ip().to_string()],
                ..Default::default()
            },
        );
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"unknown_method"}"#;

        for _ in 0..3 {
            let (_, body) = send(&router, Some("application/json"), request).await;
            assert!(body.contains("Method not found"), "{body}");
            send(&router, Some("application/json"), "not-json").await;
        }
        // Without the rule the blocklist applies, so the client is served only
        // if no policy blocked it.
        traffic_controller
            .remove_access_rule(AccessRuleList::Allow, &CLIENT.ip().to_string())
            .unwrap();
        assert!(
            traffic_controller
                .check(&Some(CLIENT.ip()), &None)
                .is_served()
        );
    }

    /// A module with one method, `test_simulate`, that reserves simulation gas
    /// as the simulation methods do, and answers whether it reserved any.
    fn simulating_methods() -> Methods {
        let mut module = jsonrpsee::RpcModule::new(());
        module
            .register_method("test_simulate", |_, _, extensions| {
                reserve_simulation_gas(extensions)
                    .map(|reservation| reservation.is_some())
                    .map_err(jsonrpsee::core::ClientError::from)
                    .map_err(iota_json_rpc_api::error_object_from_rpc)
            })
            .unwrap();
        module.into()
    }

    /// A gas limit of 100 with the computation cap of
    /// [`router_with_policy_config_and_methods`], so that one held reservation
    /// takes the whole balance, and an error policy that blocks a client on its
    /// first error.
    fn gas_limited(access_rules: AccessRules) -> PolicyConfig {
        PolicyConfig {
            error_policy_type: PolicyType::TestNConnIP(1),
            simulation_gas_limit: Some(SimulationGasLimitConfig {
                gas_per_sec: 1,
                burst_gas: 100,
            }),
            access_rules,
            ..Default::default()
        }
    }

    /// A method that reserves simulation gas, as the simulation methods do,
    /// is refused with `ServerIsBusy` and the time to retry once its client is
    /// over the limit, and the refusal does not feed the error policy, so the
    /// client is not blocked.
    #[tokio::test]
    async fn a_simulation_over_the_gas_limit_is_refused_as_busy() {
        let (router, traffic_controller) = router_with_policy_config_and_methods(
            gas_limited(AccessRules::default()),
            simulating_methods(),
        );
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"test_simulate"}"#;

        // Hold the whole balance, so that the request finds none left.
        let _held = traffic_controller
            .reserve_simulation_gas(Some(CLIENT.ip()), ClientAccess::Limited)
            .unwrap();
        let (_, body) = send(&router, Some("application/json"), request).await;
        assert!(
            body.contains(&ErrorCode::ServerIsBusy.code().to_string())
                && body.contains("simulation gas limit reached, retry in "),
            "{body}"
        );
        assert!(
            traffic_controller
                .check(&Some(CLIENT.ip()), &None)
                .is_served()
        );
    }

    /// The decision of the traffic controller reaches the simulation methods,
    /// so a client that an allow rule serves has no gas limit.
    #[tokio::test]
    async fn an_allowed_client_reserves_no_simulation_gas() {
        let (router, traffic_controller) = router_with_policy_config_and_methods(
            gas_limited(AccessRules {
                allow: vec![CLIENT.ip().to_string()],
                ..Default::default()
            }),
            simulating_methods(),
        );
        // Hold the whole balance of the client's address.
        let _held = traffic_controller
            .reserve_simulation_gas(Some(CLIENT.ip()), ClientAccess::Limited)
            .unwrap();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"test_simulate"}"#;
        let (_, body) = send(&router, Some("application/json"), request).await;
        assert!(body.contains(r#""result":false"#), "{body}");
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
                !traffic_controller
                    .check(&Some(CLIENT.ip()), &None)
                    .is_served(),
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
        assert!(
            !traffic_controller
                .check(&Some(CLIENT.ip()), &None)
                .is_served()
        );
    }

    #[test]
    fn transaction_not_found_has_zero_error_weight() {
        assert_eq!(
            normalize(ErrorCode::ServerError(TRANSACTION_NOT_FOUND_ERROR_CODE)),
            Weight::zero()
        );
    }

    #[test]
    fn a_busy_server_has_zero_error_weight() {
        // `handle_traffic_resp` reads the code of a response back as a number.
        assert_eq!(
            normalize(ErrorCode::from(ErrorCode::ServerIsBusy.code())),
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
                (callback)(id, params, max_response_body_size as usize, extensions)
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
                    extensions,
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
    extensions: Extensions,
    max_response_body_size: u32,
    request_start: L::Instant,
}

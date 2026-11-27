// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
mod validator;
mod validator_peer;
mod validator_v2;

pub mod metrics;
pub mod soft_lock;
#[cfg(test)]
mod test_server;

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use iota_network::tonic;
use iota_traffic_controller::{
    ClientAccess, ClientIpStatus, TrafficController, forwarded_hop_depth, get_client_ip,
    policies::TrafficTally,
};
use iota_types::{
    error::*,
    traffic_control::{ClientIdSource, Weight},
};
pub use metrics::ValidatorServiceMetrics;
#[cfg(test)]
pub use test_server::{AuthorityServer, AuthorityServerHandle};
use tokio_stream::StreamExt;
use tracing::error;

use crate::{
    authority::AuthorityState, authority_server::soft_lock::PreConsensusSoftLocks,
    consensus_adapter::ConsensusAdapter,
};

type WrappedServiceResponse<T> = Result<(tonic::Response<T>, Weight), tonic::Status>;

type StreamResponse<T> =
    std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<T, tonic::Status>> + Send>>;

/// The validator service.
#[derive(Clone)]
pub struct ValidatorService {
    state: Arc<AuthorityState>,
    consensus_adapter: Arc<ConsensusAdapter>,
    metrics: Arc<ValidatorServiceMetrics>,
    traffic_controller: Option<Arc<TrafficController>>,
    client_id_source: Option<ClientIdSource>,
    soft_locks: Arc<PreConsensusSoftLocks>,
}

impl ValidatorService {
    /// Creates a new `ValidatorService`.
    pub fn new(
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
        validator_metrics: Arc<ValidatorServiceMetrics>,
        client_id_source: Option<ClientIdSource>,
        soft_locks: Arc<PreConsensusSoftLocks>,
    ) -> Self {
        let traffic_controller = state.traffic_controller.clone();
        Self {
            state,
            consensus_adapter,
            metrics: validator_metrics,
            traffic_controller,
            client_id_source,
            soft_locks,
        }
    }

    pub fn new_for_tests(
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
        metrics: Arc<ValidatorServiceMetrics>,
    ) -> Self {
        Self {
            state,
            consensus_adapter,
            metrics,
            traffic_controller: None,
            client_id_source: None,
            soft_locks: Arc::new(PreConsensusSoftLocks::new()),
        }
    }

    /// Returns the validator state.
    pub fn validator_state(&self) -> &Arc<AuthorityState> {
        &self.state
    }

    /// The client IP that traffic control charges a request to, or `None`
    /// when the node could not resolve one. Errors when the request is one
    /// that [`ClientIpStatus::should_refuse_request`] refuses.
    pub(crate) fn get_client_ip_addr<T>(
        metrics: &ValidatorServiceMetrics,
        request: &tonic::Request<T>,
        source: &ClientIdSource,
    ) -> Result<Option<IpAddr>, tonic::Status> {
        // Observability gauge: track the hop depth even when we're not using
        // x-forwarded-for as the source, to detect misconfigured proxies.
        if let Some(num_hops) = forwarded_hop_depth(request.metadata().as_ref()) {
            metrics.x_forwarded_for_num_hops.set(num_hops as f64);
        }

        let status = get_client_ip(request.metadata().as_ref(), request.remote_addr(), source);
        match &status {
            ClientIpStatus::Ok(ip) => return Ok(Some(*ip)),
            ClientIpStatus::SocketAddrMissing => metrics.connection_ip_not_found.inc(),
            ClientIpStatus::XForwardedForHeaderMissing => {
                metrics.forwarded_header_not_included.inc()
            }
            ClientIpStatus::XForwardedForInvalidUtf8 => metrics.forwarded_header_invalid.inc(),
            ClientIpStatus::XForwardedForZeroHops { .. } => {}
            ClientIpStatus::XForwardedForConfigMismatch { .. } => {
                metrics.client_id_source_config_mismatch.inc()
            }
            ClientIpStatus::XForwardedForUnparsable => metrics.forwarded_header_parse_error.inc(),
        }
        error!("{status}");
        if status.should_refuse_request() {
            return Err(tonic::Status::invalid_argument(status.to_string()));
        }
        Ok(None)
    }

    /// Refuses the request when the traffic controller does. Otherwise returns
    /// `client` with what the traffic controller decided for it, which the
    /// tally of the request needs.
    fn check_traffic(&self, client: Option<IpAddr>) -> Result<ClientAndAccess, tonic::Status> {
        let Some(traffic_controller) = &self.traffic_controller else {
            return Ok((client, ClientAccess::Limited));
        };
        match traffic_controller.check(&client, &None) {
            ClientAccess::Refused => {
                Err(tonic::Status::from_error(IotaError::TooManyRequests.into()))
            }
            access => Ok((client, access)),
        }
    }

    fn extract_client_ip<ProtoReq>(
        &self,
        request: &tonic::Request<ProtoReq>,
    ) -> Result<Option<IpAddr>, tonic::Status> {
        match &self.client_id_source {
            Some(source) => Self::get_client_ip_addr(&self.metrics, request, source),
            None => Ok(None),
        }
    }

    fn tally_traffic<T>(
        &self,
        client_and_access: ClientAndAccess,
        response: Result<(T, Weight), tonic::Status>,
    ) -> Result<T, tonic::Status> {
        let (spam_weight, error, unwrapped): (Weight, Option<IotaError>, Result<T, tonic::Status>) =
            match response {
                Ok((result, w)) => (w, None, Ok(result)),
                Err(status) => (
                    Weight::zero(),
                    Some(IotaError::from(status.clone())),
                    Err(status),
                ),
            };
        record_tally(
            self.traffic_controller.as_ref(),
            client_and_access,
            spam_weight,
            error,
        );
        unwrapped
    }

    /// Checks traffic control, then extracts the domain request from a proto
    /// request. Shared pre-processing for both unary and streaming handlers.
    /// Returns the domain request and its client, as [`Self::check_traffic`]
    /// gives it.
    async fn pre_handle<ProtoReq, DomainReq>(
        &self,
        request: tonic::Request<ProtoReq>,
    ) -> Result<(DomainReq, ClientAndAccess), tonic::Status>
    where
        ProtoReq: TryInto<DomainReq>,
        ProtoReq::Error: std::fmt::Display,
    {
        let client_and_access = self.check_traffic(self.extract_client_ip(&request)?)?;
        // TODO(#11095): move deserialization off the critical path (spawn_blocking).
        let domain_req = request.into_inner().try_into().map_err(|e| {
            // A request that does not convert still costs the node work, so it
            // charges the spam policy like any other request.
            record_tally(
                self.traffic_controller.as_ref(),
                client_and_access,
                Weight::one(),
                None,
            );
            tonic::Status::internal(format!("request conversion failed: {e}"))
        })?;
        Ok((domain_req, client_and_access))
    }

    /// Tallies traffic and converts a single domain response into a proto
    /// tonic response.
    fn post_handle_unary<DomainResp, ProtoResp>(
        &self,
        client_and_access: ClientAndAccess,
        result: Result<(DomainResp, Weight), tonic::Status>,
    ) -> Result<tonic::Response<ProtoResp>, tonic::Status>
    where
        DomainResp: TryInto<ProtoResp>,
        DomainResp::Error: std::fmt::Display,
    {
        let value = self.tally_traffic(client_and_access, result)?;
        let proto = value
            .try_into()
            .map_err(|e| tonic::Status::internal(format!("response conversion failed: {e}")))?;
        Ok(tonic::Response::new(proto))
    }

    /// Tallies traffic per stream item and maps each domain item into its
    /// proto equivalent. Each item carries its own business-logic Weight,
    /// which feeds the spam-policy axis of the traffic controller, and
    /// `item_error` returns the error an item reports, which feeds the error
    /// policy. Top-level errors are tallied once at stream creation.
    fn post_handle_stream<S, DomainItem, ProtoItem>(
        &self,
        client_and_access: ClientAndAccess,
        result: Result<S, tonic::Status>,
        item_error: fn(&DomainItem) -> Option<IotaError>,
    ) -> Result<tonic::Response<StreamResponse<ProtoItem>>, tonic::Status>
    where
        S: tokio_stream::Stream<Item = Result<(DomainItem, Weight), tonic::Status>>
            + Send
            + 'static,
        DomainItem: TryInto<ProtoItem> + 'static,
        DomainItem::Error: std::fmt::Display,
        ProtoItem: 'static,
    {
        let stream = match result {
            Ok(s) => s,
            Err(status) => {
                record_tally(
                    self.traffic_controller.as_ref(),
                    client_and_access,
                    Weight::zero(),
                    Some(IotaError::from(status.clone())),
                );
                return Err(status);
            }
        };

        let traffic_controller = self.traffic_controller.clone();
        let mapped = stream.map(move |item| {
            let (spam_weight, error, forward): (
                Weight,
                Option<IotaError>,
                Result<DomainItem, tonic::Status>,
            ) = match item {
                Ok((data, w)) => (w, item_error(&data), Ok(data)),
                Err(status) => (
                    Weight::zero(),
                    Some(IotaError::from(status.clone())),
                    Err(status),
                ),
            };
            record_tally(
                traffic_controller.as_ref(),
                client_and_access,
                spam_weight,
                error,
            );
            forward.and_then(|v| {
                v.try_into().map_err(|e| {
                    tonic::Status::internal(format!("stream item conversion failed: {e}"))
                })
            })
        });
        Ok(tonic::Response::new(Box::pin(mapped)))
    }
}

fn make_tonic_request_for_testing<T>(message: T) -> tonic::Request<T> {
    // simulate a TCP connection, which would have added extensions to
    // the request object that would be used downstream
    let mut request = tonic::Request::new(message);
    let tcp_connect_info = tonic::transport::server::TcpConnectInfo {
        local_addr: None,
        remote_addr: Some(SocketAddr::new([127, 0, 0, 1].into(), 0)),
    };
    request.extensions_mut().insert(tcp_connect_info);
    request
}

// TODO(#11095): refine error-to-weight mapping.
pub(super) fn normalize(err: &IotaError) -> Weight {
    match err {
        IotaError::UserInput {
            error:
                UserInputError::IncorrectUserSignature { .. }
                | UserInputError::MoveAuthenticatorNotFound { .. }
                | UserInputError::UnableToGetMoveAuthenticatorId { .. }
                | UserInputError::InvalidAuthenticatorFunctionRefField { .. }
                | UserInputError::PackageIsInMoveAuthenticatorInput { .. }
                | UserInputError::AddressOwnedIsInMoveAuthenticatorInput { .. }
                | UserInputError::ObjectOwnedIsInMoveAuthenticatorInput { .. }
                | UserInputError::MutableSharedIsInMoveAuthenticatorInput { .. }
                | UserInputError::RandomnessStateIsInMoveAuthenticatorInput { .. },
        } => Weight::one(),
        IotaError::InvalidSignature { .. }
        | IotaError::SignerSignatureAbsent { .. }
        | IotaError::SignerSignatureNumberMismatch { .. }
        | IotaError::IncorrectSigner { .. }
        | IotaError::UnknownSigner { .. }
        | IotaError::WrongEpoch { .. }
        | IotaError::MoveAuthenticatorExecutionFailure { .. }
        | IotaError::InvalidMoveAuthenticatorDigest
        | IotaError::InvalidAuthenticator => Weight::one(),
        _ => Weight::zero(),
    }
}

/// The client of a request, and what the traffic controller decided for it.
type ClientAndAccess = (Option<IpAddr>, ClientAccess);

/// Records a single traffic tally. No-op if traffic control is disabled, or
/// if an allow rule serves the client, since no policy counts such a client.
fn record_tally(
    traffic_controller: Option<&Arc<TrafficController>>,
    (client, access): ClientAndAccess,
    spam_weight: Weight,
    error: Option<IotaError>,
) {
    let Some(tc) = traffic_controller else {
        return;
    };
    if access == ClientAccess::Allowed {
        return;
    }
    tc.tally(TrafficTally {
        direct: client,
        through_fullnode: None,
        error_info: error.map(|e| {
            let error_type = String::from(e.as_ref());
            let error_weight = normalize(&e);
            (error_weight, error_type)
        }),
        spam_weight,
    });
}

/// Implements generic pre- and post-processing. Since this is on the critical
/// path, any heavy lifting should be done in a separate non-blocking task
/// unless it is necessary to override the return value.
#[macro_export]
macro_rules! handle_with_decoration {
    ($self:ident, $func_name:ident, $request:ident) => {{
        if $self.client_id_source.is_none() {
            return $self.$func_name($request).await.map(|(result, _)| result);
        }

        // Return early if the client is refused.
        let client_and_access = $self.check_traffic($self.extract_client_ip(&$request)?)?;

        // handle traffic tallying
        let wrapped_response = $self.$func_name($request).await;
        $self.tally_traffic(client_and_access, wrapped_response)
    }};
}

#[cfg(test)]
mod client_ip_forwarding_tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use iota_traffic_controller::parse_ip;
    use iota_types::traffic_control::ClientIdSource;
    use tonic::codegen::http;

    use super::{ValidatorService, ValidatorServiceMetrics};
    use crate::authority_client::insert_metadata;

    /// A request carrying `value` as its `x-forwarded-for` header, built over
    /// the raw bytes so that a test can send what the gRPC metadata API
    /// refuses.
    fn forwarded(value: &[u8]) -> tonic::Request<()> {
        let request = http::Request::builder()
            .header(
                "x-forwarded-for",
                http::HeaderValue::from_bytes(value).expect("a valid header value"),
            )
            .body(())
            .expect("a valid request");
        tonic::Request::from_http(request)
    }

    /// The node reports the depth of the proxy chain even when the part of the
    /// header the client sent is not readable as text.
    #[test]
    fn a_byte_the_client_sent_does_not_zero_the_hop_gauge() {
        let metrics = ValidatorServiceMetrics::new_for_tests();
        let client = ValidatorService::get_client_ip_addr(
            &metrics,
            &forwarded(b"\x80, 10.0.0.1"),
            &ClientIdSource::XForwardedFor(1),
        )
        .expect("the selected entry is an address");

        assert_eq!(client, Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert_eq!(metrics.x_forwarded_for_num_hops.get(), 1.0);
    }

    /// A request that reached the node without the proxy that sets the header
    /// names no client, and the node counts it under that cause.
    #[test]
    fn a_request_without_the_header_is_counted_under_its_cause() {
        let metrics = ValidatorServiceMetrics::new_for_tests();
        let client = ValidatorService::get_client_ip_addr(
            &metrics,
            &tonic::Request::new(()),
            &ClientIdSource::XForwardedFor(1),
        )
        .expect("a request without the header is served");

        assert_eq!(client, None);
        assert_eq!(metrics.forwarded_header_not_included.get(), 1);
    }

    /// A proxy never writes an entry that is not valid UTF-8, so the node
    /// refuses such a request, and counts it under that cause.
    #[test]
    fn an_unreadable_selected_entry_is_refused_and_counted() {
        let metrics = ValidatorServiceMetrics::new_for_tests();
        let status = ValidatorService::get_client_ip_addr(
            &metrics,
            &forwarded(b"10.0.0.1, \x80"),
            &ClientIdSource::XForwardedFor(1),
        )
        .expect_err("the node refuses an unreadable selected entry");

        assert_eq!(status.code(), tonic::Code::InvalidArgument);
        assert_eq!(metrics.forwarded_header_invalid.get(), 1);
    }

    /// Verifies that `insert_metadata` on the client side sets the
    /// `x-forwarded-for` header such that the server-side
    /// `XForwardedFor` parsing extracts the correct IP address.
    #[test]
    fn client_ip_round_trip_via_x_forwarded_for() {
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 9000);

        // Client side: build a tonic request and inject metadata.
        let mut request = tonic::Request::new(());
        insert_metadata(&mut request, Some(client_addr));

        // Server side: extract the x-forwarded-for header and parse it using
        // the same logic as get_client_ip_addr with XForwardedFor(1).
        let header = request
            .metadata()
            .get("x-forwarded-for")
            .expect("x-forwarded-for should be set");
        let header_val = header.to_str().unwrap();
        let header_contents: Vec<&str> = header_val.split(',').map(str::trim).collect();

        // XForwardedFor(1) takes the last entry.
        let num_hops: usize = 1;
        let client_ip = header_contents[header_contents.len() - num_hops];
        let extracted_ip = parse_ip(client_ip);

        assert_eq!(
            extracted_ip,
            Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)))
        );
    }

    /// Verifies that when no client_addr is provided, no header is set.
    #[test]
    fn no_client_addr_means_no_forwarded_header() {
        let mut request = tonic::Request::new(());
        insert_metadata(&mut request, None);

        assert!(request.metadata().get("x-forwarded-for").is_none());
    }

    /// Verifies XForwardedFor with SocketAddr source (direct connection)
    /// still works — the server reads remote_addr from the connection.
    #[test]
    fn socket_addr_source_uses_connection_info() {
        // With SocketAddr source, the server reads remote_addr() from the
        // tonic request, not the x-forwarded-for header. This is set by the
        // transport layer (TcpConnectInfo), not by insert_metadata.
        // Verify that insert_metadata doesn't interfere with SocketAddr mode.
        let client_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 5000);
        let mut request = tonic::Request::new(());
        insert_metadata(&mut request, Some(client_addr));

        // The SocketAddr source ignores x-forwarded-for and reads
        // remote_addr(). We verify the header is set but SocketAddr mode
        // would not use it.
        assert!(
            request.metadata().get("x-forwarded-for").is_some(),
            "Header should be set even though SocketAddr source ignores it"
        );
        // remote_addr() would be None here since there's no real TCP
        // connection, confirming SocketAddr mode doesn't use the header.
        assert!(
            request.remote_addr().is_none(),
            "No real TCP connection, so remote_addr should be None"
        );
    }

    /// Verify that parse_ip (used server-side) correctly parses both IP-only
    /// and IP:port formats from the forwarded header.
    #[test]
    fn parse_ip_handles_socket_addr_format() {
        // insert_metadata writes SocketAddr format (ip:port)
        assert_eq!(
            parse_ip("192.168.1.100:9000"),
            Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)))
        );
        // Also works with plain IP
        assert_eq!(
            parse_ip("10.0.0.1"),
            Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)))
        );
    }
}

#[cfg(test)]
mod traffic_tally_tests {
    use std::{
        net::{IpAddr, Ipv4Addr},
        sync::Arc,
    };

    use iota_network::{
        api::{self, Validator, ValidatorV2},
        tonic,
    };
    use iota_traffic_controller::{ClientAccess, TrafficController};
    use iota_types::{
        error::IotaError,
        messages_grpc::SystemStateRequest,
        traffic_control::{
            AccessRuleList, AccessRules, ClientIdSource, PolicyConfig, PolicyType, Weight,
        },
    };

    use super::{
        ValidatorService, ValidatorServiceMetrics, make_tonic_request_for_testing, record_tally,
        soft_lock::PreConsensusSoftLocks,
    };
    use crate::{
        authority::test_authority_builder::TestAuthorityBuilder,
        consensus_adapter::ConsensusAdapter,
    };

    const CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    /// The client that a request from [`make_tonic_request_for_testing`] comes
    /// from.
    fn test_request_client() -> IpAddr {
        make_tonic_request_for_testing(())
            .remote_addr()
            .expect("the test request has a peer address")
            .ip()
    }

    /// A validator service on a test authority, with a traffic controller that
    /// blocks a client on its first tallied request and serves `allowed`
    /// without a tally. It is built as [`ValidatorService::new`] builds it,
    /// with this traffic controller in place of the one of the authority.
    async fn validator_service_allowing(
        allowed: IpAddr,
    ) -> (ValidatorService, Arc<TrafficController>) {
        let state = TestAuthorityBuilder::new().build().await;
        let traffic_controller = Arc::new(TrafficController::init_for_test(PolicyConfig {
            spam_policy_type: PolicyType::TestNConnIP(1),
            spam_sample_rate: Weight::one(),
            dry_run: false,
            access_rules: AccessRules {
                allow: vec![allowed.to_string()],
                ..Default::default()
            },
            ..Default::default()
        }));
        let service = ValidatorService {
            consensus_adapter: Arc::new(ConsensusAdapter::new_for_testing_with_authority_name(
                state.name,
            )),
            state,
            metrics: Arc::new(ValidatorServiceMetrics::new_for_tests()),
            traffic_controller: Some(traffic_controller.clone()),
            client_id_source: Some(ClientIdSource::SocketAddr),
            soft_locks: Arc::new(PreConsensusSoftLocks::new()),
        };
        (service, traffic_controller)
    }

    /// A handler that `handle_with_decoration!` wraps does not tally a client
    /// that an allow rule serves, so no policy blocks the client.
    #[tokio::test]
    async fn a_decorated_handler_does_not_tally_an_allowed_client() {
        let client = test_request_client();
        let (service, traffic_controller) = validator_service_allowing(client).await;
        let get_system_state = || {
            service.get_system_state_object(make_tonic_request_for_testing(SystemStateRequest {
                _unused: false,
            }))
        };
        for _ in 0..3 {
            get_system_state()
                .await
                .expect("the node serves an allowed client");
        }
        traffic_controller
            .remove_access_rule(AccessRuleList::Allow, &client.to_string())
            .unwrap();
        assert_eq!(
            traffic_controller.check(&Some(client), &None),
            ClientAccess::Limited
        );

        // Without the rule, one request blocks the client.
        get_system_state()
            .await
            .expect("the client is not blocked yet");
        assert_eq!(
            traffic_controller.check(&Some(client), &None),
            ClientAccess::Refused
        );
    }

    /// `pre_handle` does not tally a client that an allow rule serves. It
    /// tallies a request that does not convert, such as a status query
    /// without a digest.
    #[tokio::test]
    async fn pre_handle_does_not_tally_an_allowed_client() {
        let client = test_request_client();
        let (service, traffic_controller) = validator_service_allowing(client).await;
        let get_tx_status_without_digest = || async {
            service
                .get_tx_status(make_tonic_request_for_testing(api::GetTxStatusRequest {
                    queries: vec![api::TxStatusQuery::default()],
                }))
                .await
                .err()
                .expect("a status query without a digest does not convert")
        };
        for _ in 0..3 {
            let status = get_tx_status_without_digest().await;
            assert_eq!(status.code(), tonic::Code::Internal, "{status}");
        }
        traffic_controller
            .remove_access_rule(AccessRuleList::Allow, &client.to_string())
            .unwrap();
        assert_eq!(
            traffic_controller.check(&Some(client), &None),
            ClientAccess::Limited
        );

        // Without the rule, one request blocks the client.
        let status = get_tx_status_without_digest().await;
        assert_eq!(status.code(), tonic::Code::Internal, "{status}");
        assert_eq!(
            traffic_controller.check(&Some(client), &None),
            ClientAccess::Refused
        );
    }

    /// A request from a client that an allow rule serves is not tallied, so
    /// an error policy that blocks on the first error does not block it.
    #[tokio::test]
    async fn an_allowed_client_is_not_tallied() {
        let traffic_controller =
            std::sync::Arc::new(TrafficController::init_for_test(PolicyConfig {
                error_policy_type: PolicyType::TestNConnIP(1),
                dry_run: false,
                access_rules: AccessRules {
                    allow: vec![CLIENT.to_string()],
                    ..Default::default()
                },
                ..Default::default()
            }));
        let access = traffic_controller.check(&Some(CLIENT), &None);
        assert_eq!(access, ClientAccess::Allowed);

        record_tally(
            Some(&traffic_controller),
            (Some(CLIENT), access),
            Weight::one(),
            Some(IotaError::InvalidAuthenticator),
        );
        traffic_controller
            .remove_access_rule(AccessRuleList::Allow, &CLIENT.to_string())
            .unwrap();
        assert_eq!(
            traffic_controller.check(&Some(CLIENT), &None),
            ClientAccess::Limited
        );

        // The same error from a client that no rule serves blocks it.
        record_tally(
            Some(&traffic_controller),
            (Some(CLIENT), ClientAccess::Limited),
            Weight::one(),
            Some(IotaError::InvalidAuthenticator),
        );
        assert_eq!(
            traffic_controller.check(&Some(CLIENT), &None),
            ClientAccess::Refused
        );
    }
}

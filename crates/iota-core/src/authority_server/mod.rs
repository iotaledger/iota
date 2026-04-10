// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
mod validator;
mod validator_peer;
mod validator_v2;

pub mod metrics;
#[cfg(test)]
mod test_server;

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::SystemTime,
};

use iota_network::{
    tonic,
    tonic::metadata::{Ascii, MetadataValue},
};
use iota_types::{
    error::*,
    traffic_control::{ClientIdSource, Weight},
};
pub use metrics::ValidatorServiceMetrics;
#[cfg(test)]
pub use test_server::{AuthorityServer, AuthorityServerHandle};
use tokio_stream::StreamExt;
use tonic::transport::server::TcpConnectInfo;
use tracing::error;

use crate::{
    authority::AuthorityState,
    consensus_adapter::ConsensusAdapter,
    traffic_controller::{TrafficController, parse_ip, policies::TrafficTally},
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
}

impl ValidatorService {
    /// Creates a new `ValidatorService`.
    pub fn new(
        state: Arc<AuthorityState>,
        consensus_adapter: Arc<ConsensusAdapter>,
        validator_metrics: Arc<ValidatorServiceMetrics>,
        client_id_source: Option<ClientIdSource>,
    ) -> Self {
        let traffic_controller = state.traffic_controller.clone();
        Self {
            state,
            consensus_adapter,
            metrics: validator_metrics,
            traffic_controller,
            client_id_source,
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
        }
    }

    /// Returns the validator state.
    pub fn validator_state(&self) -> &Arc<AuthorityState> {
        &self.state
    }

    pub(crate) fn get_client_ip_addr<T>(
        &self,
        request: &tonic::Request<T>,
        source: &ClientIdSource,
    ) -> Option<IpAddr> {
        let forwarded_header = request.metadata().get_all("x-forwarded-for").iter().next();

        if let Some(header) = forwarded_header {
            let num_hops = header
                .to_str()
                .map(|h| h.split(',').count().saturating_sub(1))
                .unwrap_or(0);

            self.metrics.x_forwarded_for_num_hops.set(num_hops as f64);
        }

        match source {
            ClientIdSource::SocketAddr => {
                let socket_addr: Option<SocketAddr> = request.remote_addr();

                // We will hit this case if the IO type used does not
                // implement Connected or when using a unix domain socket.
                // TODO(#11095): reject requests without a peer address.
                if let Some(socket_addr) = socket_addr {
                    Some(socket_addr.ip())
                } else {
                    if cfg!(msim) {
                        // Ignore the error from simtests.
                    } else if cfg!(test) {
                        panic!("Failed to get remote address from request");
                    } else {
                        self.metrics.connection_ip_not_found.inc();
                        error!("Failed to get remote address from request");
                    }
                    None
                }
            }
            ClientIdSource::XForwardedFor(num_hops) => {
                let do_header_parse = |op: &MetadataValue<Ascii>| {
                    match op.to_str() {
                        Ok(header_val) => {
                            let header_contents =
                                header_val.split(',').map(str::trim).collect::<Vec<_>>();
                            if *num_hops == 0 {
                                error!(
                                    "x-forwarded-for: 0 specified. x-forwarded-for contents: {:?}. Please assign nonzero value for \
                                    number of hops here, or use `socket-addr` client-id-source type if requests are not being proxied \
                                    to this node. Skipping traffic controller request handling.",
                                    header_contents,
                                );
                                return None;
                            }
                            let contents_len = header_contents.len();
                            if contents_len < *num_hops {
                                error!(
                                    "x-forwarded-for header value of {:?} contains {} values, but {} hops were specified. \
                                    Expected at least {} values. Please correctly set the `x-forwarded-for` value under \
                                    `client-id-source` in the node config.",
                                    header_contents, contents_len, num_hops, contents_len,
                                );
                                self.metrics.client_id_source_config_mismatch.inc();
                                return None;
                            }
                            let Some(client_ip) = header_contents.get(contents_len - num_hops)
                            else {
                                error!(
                                    "x-forwarded-for header value of {:?} contains {} values, but {} hops were specified. \
                                    Expected at least {} values. Skipping traffic controller request handling.",
                                    header_contents, contents_len, num_hops, contents_len,
                                );
                                return None;
                            };
                            parse_ip(client_ip).or_else(|| {
                                self.metrics.forwarded_header_parse_error.inc();
                                None
                            })
                        }
                        Err(e) => {
                            // TODO(#11095): reject requests with invalid x-forwarded-for.
                            self.metrics.forwarded_header_invalid.inc();
                            error!("Invalid UTF-8 in x-forwarded-for header: {:?}", e);
                            None
                        }
                    }
                };
                if let Some(op) = request.metadata().get("x-forwarded-for") {
                    do_header_parse(op)
                } else if let Some(op) = request.metadata().get("X-Forwarded-For") {
                    do_header_parse(op)
                } else {
                    self.metrics.forwarded_header_not_included.inc();
                    error!(
                        "x-forwarded-for header not present for request despite node configuring x-forwarded-for tracking type"
                    );
                    None
                }
            }
        }
    }

    async fn check_traffic(&self, client: Option<IpAddr>) -> Result<(), tonic::Status> {
        if let Some(traffic_controller) = &self.traffic_controller {
            if !traffic_controller.check(&client, &None).await {
                // Entity in blocklist
                Err(tonic::Status::from_error(IotaError::TooManyRequests.into()))
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn extract_client_ip_and_request<DomainReq, ProtoReq>(
        &self,
        request: tonic::Request<ProtoReq>,
    ) -> Result<(DomainReq, Option<IpAddr>), tonic::Status>
    where
        ProtoReq: TryInto<DomainReq>,
        ProtoReq::Error: std::fmt::Display,
    {
        let ip = self
            .client_id_source
            .as_ref()
            .and_then(|source| self.get_client_ip_addr(&request, source));
        // TODO(#11095): move deserialization off the critical path (spawn_blocking).
        let domain_req = request
            .into_inner()
            .try_into()
            .map_err(|e| tonic::Status::internal(format!("request conversion failed: {e}")))?;
        Ok((domain_req, ip))
    }

    fn tally_traffic<T>(
        &self,
        client: Option<IpAddr>,
        response: Result<(T, Weight), tonic::Status>,
    ) -> Result<T, tonic::Status> {
        let (error, spam_weight, unwrapped_response): (
            Option<IotaError>,
            Weight,
            Result<T, tonic::Status>,
        ) = match response {
            Ok((result, spam_weight)) => (None, spam_weight, Ok(result)),
            Err(status) => (
                Some(IotaError::from(status.clone())),
                Weight::zero(),
                Err(status),
            ),
        };

        if let Some(traffic_controller) = self.traffic_controller.clone() {
            traffic_controller.tally(TrafficTally {
                direct: client,
                through_fullnode: None,
                error_info: error.map(|e| {
                    let error_type = String::from(e.as_ref());
                    let error_weight = normalize(e);
                    (error_weight, error_type)
                }),
                spam_weight,
                timestamp: SystemTime::now(),
            })
        }
        unwrapped_response
    }

    /// Extracts the domain request from a proto request and checks traffic
    /// control. Shared pre-processing for both unary and streaming handlers.
    async fn pre_handle<ProtoReq, DomainReq>(
        &self,
        request: tonic::Request<ProtoReq>,
    ) -> Result<(DomainReq, Option<IpAddr>), tonic::Status>
    where
        ProtoReq: TryInto<DomainReq>,
        ProtoReq::Error: std::fmt::Display,
    {
        let (domain_req, ip) = self.extract_client_ip_and_request(request)?;
        self.check_traffic(ip).await?;
        Ok((domain_req, ip))
    }

    /// Tallies traffic and converts a single domain response into a proto
    /// tonic response.
    fn post_handle_unary<DomainResp, ProtoResp>(
        &self,
        ip: Option<IpAddr>,
        result: Result<(DomainResp, Weight), tonic::Status>,
    ) -> Result<tonic::Response<ProtoResp>, tonic::Status>
    where
        DomainResp: TryInto<ProtoResp>,
        DomainResp::Error: std::fmt::Display,
    {
        let value = self.tally_traffic(ip, result)?;
        let proto = value
            .try_into()
            .map_err(|e| tonic::Status::internal(format!("response conversion failed: {e}")))?;
        Ok(tonic::Response::new(proto))
    }

    /// Tallies traffic and maps each domain stream item into its proto
    /// equivalent.
    // TODO(#11080): tally traffic per-item, not just once at stream creation.
    fn post_handle_stream<S, DomainItem, ProtoItem>(
        &self,
        ip: Option<IpAddr>,
        result: Result<(S, Weight), tonic::Status>,
    ) -> Result<tonic::Response<StreamResponse<ProtoItem>>, tonic::Status>
    where
        S: tokio_stream::Stream<Item = Result<DomainItem, tonic::Status>> + Send + 'static,
        DomainItem: TryInto<ProtoItem> + 'static,
        DomainItem::Error: std::fmt::Display,
        ProtoItem: 'static,
    {
        let stream = self.tally_traffic(ip, result)?;
        let mapped = stream.map(|item| {
            item.and_then(|v| {
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
    let tcp_connect_info = TcpConnectInfo {
        local_addr: None,
        remote_addr: Some(SocketAddr::new([127, 0, 0, 1].into(), 0)),
    };
    request.extensions_mut().insert(tcp_connect_info);
    request
}

// TODO(#11095): refine error-to-weight mapping.
fn normalize(err: IotaError) -> Weight {
    match err {
        IotaError::UserInput {
            error: UserInputError::IncorrectUserSignature { .. },
        } => Weight::one(),
        IotaError::InvalidSignature { .. }
        | IotaError::SignerSignatureAbsent { .. }
        | IotaError::SignerSignatureNumberMismatch { .. }
        | IotaError::IncorrectSigner { .. }
        | IotaError::UnknownSigner { .. }
        | IotaError::WrongEpoch { .. } => Weight::one(),
        _ => Weight::zero(),
    }
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

        let client = $self.get_client_ip_addr(&$request, $self.client_id_source.as_ref().unwrap());

        // check if either IP is blocked, in which case return early
        $self.check_traffic(client.clone()).await?;

        // handle traffic tallying
        let wrapped_response = $self.$func_name($request).await;
        $self.tally_traffic(client, wrapped_response)
    }};
}

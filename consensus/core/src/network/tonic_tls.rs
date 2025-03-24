// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use consensus_config::{AuthorityIndex, NetworkKeyPair};
use iota_tls::AllowPublicKeys;
use tokio_rustls::rustls::{ClientConfig, ServerConfig};

use crate::context::Context;

pub(crate) fn create_rustls_server_config(
    context: &Context,
    network_keypair: NetworkKeyPair,
) -> ServerConfig {
    let allower = AllowPublicKeys::new(
        context
            .committee
            .authorities()
            .map(|(_i, a)| a.network_key.clone().into_inner())
            .collect(),
    );
    let verifier = iota_tls::ClientCertVerifier::new(allower, certificate_server_name(context));
    // TODO: refactor to use key bytes
    let self_signed_cert = iota_tls::SelfSignedCertificate::new(
        network_keypair.private_key().into_inner(),
        &certificate_server_name(context),
    );
    let tls_cert = self_signed_cert.rustls_certificate();
    let tls_private_key = self_signed_cert.rustls_private_key();
    let mut tls_config = verifier
        .rustls_server_config(vec![tls_cert], tls_private_key)
        .unwrap_or_else(|e| panic!("Failed to create TLS server config: {:?}", e));
    tls_config.alpn_protocols = vec![b"h2".to_vec()];
    tls_config
}

pub(crate) fn create_rustls_client_config(
    context: &Context,
    network_keypair: NetworkKeyPair,
    target: AuthorityIndex,
) -> ClientConfig {
    let target_public_key = context
        .committee
        .authority(target)
        .network_key
        .clone()
        .into_inner();
    let self_signed_cert = iota_tls::SelfSignedCertificate::new(
        network_keypair.private_key().into_inner(),
        &certificate_server_name(context),
    );
    let tls_cert = self_signed_cert.rustls_certificate();
    let tls_private_key = self_signed_cert.rustls_private_key();
    let mut tls_config =
        iota_tls::ServerCertVerifier::new(target_public_key, certificate_server_name(context))
            .rustls_client_config(vec![tls_cert], tls_private_key)
            .unwrap_or_else(|e| panic!("Failed to create TLS client config: {:?}", e));
    // ServerCertVerifier sets alpn for completeness, but alpn cannot be predefined
    // when using HttpsConnector from hyper-rustls, as in TonicManager.
    tls_config.alpn_protocols = vec![];
    tls_config
}

fn certificate_server_name(context: &Context) -> String {
    format!("consensus_epoch_{}", context.committee.epoch())
}

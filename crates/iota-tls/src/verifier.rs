// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::ed25519::Ed25519PublicKey;
use fastcrypto::traits::ToFromBytes;
use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};
use std::fmt::{Debug, Formatter};
use rustls::client::danger::HandshakeSignatureValid;
use rustls::{DigitallySignedStruct, DistinguishedName, Error, SignatureScheme};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, UnixTime};
use webpki::RevocationOptions;

static SUPPORTED_SIG_ALGS: &[&rustls::pki_types::AlgorithmIdentifier] = &[&webpki::alg_id::ED25519];

pub type ValidatorAllowlist = Arc<RwLock<HashSet<Ed25519PublicKey>>>;

/// The Allower trait provides an interface for callers to inject decisions
/// whether to allow a cert to be verified or not.  This does not perform actual
/// cert validation it only acts as a gatekeeper to decide if we should even
/// try.  For example, we may want to filter our actions to well known public
/// keys.
pub trait Allower: Send + Sync {
    fn allowed(&self, key: &Ed25519PublicKey) -> bool;
}

/// AllowAll will allow all public certificates to be validated, it fails open
#[derive(Clone, Default)]
pub struct AllowAll;

impl Allower for AllowAll {
    fn allowed(&self, _: &Ed25519PublicKey) -> bool {
        true
    }
}

/// HashSetAllow restricts keys to those that are found in the member set.
/// non-members will not be allowed.
#[derive(Clone, Default)]
pub struct HashSetAllow {
    inner: ValidatorAllowlist,
}

impl HashSetAllow {
    pub fn new() -> Self {
        let inner = Arc::new(RwLock::new(HashSet::new()));
        Self { inner }
    }
    /// Get a reference to the inner service
    pub fn inner(&self) -> &ValidatorAllowlist {
        &self.inner
    }

    /// Get a mutable reference to the inner service
    pub fn inner_mut(&mut self) -> &mut ValidatorAllowlist {
        &mut self.inner
    }
}

impl Allower for HashSetAllow {
    fn allowed(&self, key: &Ed25519PublicKey) -> bool {
        self.inner.read().unwrap().contains(key)
    }
}

/// A `rustls::server::ClientCertVerifier` that will ensure that every client
/// provides a valid, expected certificate and that the client's public key is
/// in the validator set.
#[derive(Clone, Debug)]
pub struct ClientCertVerifier<A: Debug> {
    allower: A,
    name: String,
}

impl<A> ClientCertVerifier<A> {
    pub fn new(allower: A, name: String) -> Self {
        Self { allower, name }
    }
}

impl<A: Allower + 'static> ClientCertVerifier<A> {
    pub fn rustls_server_config(
        self,
        certificates: Vec<CertificateDer>,
        private_key: rustls::pki_types::PrivateKeyDer,
    ) -> Result<rustls::ServerConfig, Error> {
        let mut config = rustls::ServerConfig::builder()
            .with_client_cert_verifier(Arc::new(self))
            .with_single_cert(certificates, private_key)?;
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        Ok(config)
    }
}

impl<A: Allower + Debug> rustls::server::danger::ClientCertVerifier for ClientCertVerifier<A> {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        // TODO: what to do here?
        &[]
    }

    // Verifies this is a valid ed25519 self-signed certificate
    // 1. we prepare arguments for webpki's certificate verification (following the
    //    rustls implementation) placing the public key at the root of the
    //    certificate chain (as it should be for a self-signed certificate)
    // 2. we call webpki's certificate verification
    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer,
        intermediates: &[CertificateDer],
        now: UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, Error> {
        // Step 1: Check this matches the key we expect
        let public_key = public_key_from_certificate(end_entity)?;

        if !self.allower.allowed(&public_key) {
            return Err(rustls::Error::General(format!(
                "invalid certificate: {:?} is not in the validator set",
                public_key,
            )));
        }

        // Step 2: verify the certificate signature and server name with webpki.
        verify_self_signed_cert(
            end_entity,
            intermediates,
            webpki::KeyUsage::client_auth(),
            &self.name,
            now,
        )
            .map(|_| rustls::server::danger::ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &CryptoProvider::get_default().unwrap().signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &CryptoProvider::get_default().unwrap().signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        CryptoProvider::get_default().unwrap().signature_verification_algorithms.supported_schemes()
    }
}

/// A `rustls::client::ServerCertVerifier` that ensures the client only connects with the
/// expected server.
#[derive(Clone, Debug)]
pub struct ServerCertVerifier {
    public_key: Ed25519PublicKey,
    name: String,
}

impl ServerCertVerifier {
    pub fn new(public_key: Ed25519PublicKey, name: String) -> Self {
        Self { public_key, name }
    }

    pub fn rustls_client_config(
        self,
        certificates: Vec<CertificateDer>,
        private_key: rustls::pki_types::PrivateKeyDer,
    ) -> Result<rustls::ClientConfig, Error> {
        let mut config = rustls::ClientConfig::builder().dangerous()
            .with_custom_certificate_verifier(Arc::new(self))
            .with_client_auth_cert(certificates, private_key)?;
        config.alpn_protocols = vec![b"h2".to_vec()];
        Ok(config)
    }
}

impl rustls::client::danger::ServerCertVerifier for ServerCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer,
        intermediates: &[CertificateDer],
        _server_name: &rustls::pki_types::ServerName,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, Error> {
        let public_key = public_key_from_certificate(end_entity)?;
        if public_key != self.public_key {
            return Err(Error::General(format!(
                "invalid certificate: {:?} is not the expected server public key",
                public_key,
            )));
        }

        verify_self_signed_cert(
            end_entity,
            intermediates,
            webpki::KeyUsage::server_auth(),
            &self.name,
            now,
        )
            .map(|_| rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &CryptoProvider::get_default().unwrap().signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &CryptoProvider::get_default().unwrap().signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        CryptoProvider::get_default().unwrap().signature_verification_algorithms.supported_schemes()
    }
}

// Verifies this is a valid ed25519 self-signed certificate
// 1. we prepare arguments for webpki's certificate verification (following the rustls implementation)
//    placing the public key at the root of the certificate chain (as it should be for a self-signed certificate)
// 2. we call webpki's certificate verification
fn verify_self_signed_cert(
    end_entity: &rustls::pki_types::CertificateDer,
    intermediates: &[rustls::pki_types::CertificateDer],
    usage: webpki::KeyUsage,
    name: &str,
    now: std::time::SystemTime,
) -> Result<(), rustls::Error> {
    // Check we're receiving correctly signed data with the expected key
    // Step 1: prepare arguments
    let (cert, chain, trustroots) = prepare_for_self_signed(end_entity, intermediates)?;
    let now = webpki::types::UnixTime::try_from(now).map_err(|_| rustls::Error::FailedToGetCurrentTime)?;

    // Step 2: call verification from webpki
    let cert = cert
        .verify_for_usage(SUPPORTED_SIG_ALGS, &trustroots, chain.as_slice(), now, usage, None, None)
        .map_err(pki_error)
        .map(|_| cert)?;

    // Ensure the cert is valid for the network name
    let dns_nameref = webpki::types::ServerName::try_from_ascii_str(name)
        .map_err(|_| rustls::Error::UnsupportedNameType)?;
    cert.verify_is_valid_for_subject_name(dns_nameref)
        .map_err(pki_error)
}

type CertChainAndRoots<'a> = (
    webpki::EndEntityCert<'a>,
    Vec<rustls::pki_types::CertificateDer<'a>>,
    Vec<webpki::types::TrustAnchor<'a>>,
);

// This prepares arguments for webpki, including a trust anchor which is the end
// entity of the certificate (which embodies a self-signed certificate by
// definition)
fn prepare_for_self_signed<'a>(
    end_entity: &'a rustls::pki_types::CertificateDer,
    intermediates: &'a [rustls::pki_types::CertificateDer],
) -> Result<CertChainAndRoots<'a>, rustls::Error> {
    // EE cert must appear first.
    let cert = webpki::EndEntityCert::try_from(end_entity.as_ref()).map_err(pki_error)?;

    // reinterpret the certificate as a root, materializing the self-signed policy
    let root = webpki::types::TrustAnchor::try_from_cert_der(end_entity.as_ref()).map_err(pki_error)?;

    Ok((cert, intermediates, vec![root]))
}

fn pki_error(error: webpki::Error) -> rustls::Error {
    use webpki::Error::*;
    match error {
        BadDer | BadDerTime => {
            rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding)
        }
        InvalidSignatureForPublicKey
        | UnsupportedSignatureAlgorithm
        | UnsupportedSignatureAlgorithmForPublicKey => {
            rustls::Error::InvalidCertificate(rustls::CertificateError::BadSignature)
        }
        CertNotValidForName => {
            rustls::Error::InvalidCertificate(rustls::CertificateError::NotValidForName)
        }
        e => rustls::Error::General(format!("invalid peer certificate: {e}")),
    }
}

/// Extracts the public key from a certificate.
pub fn public_key_from_certificate(
    certificate: &rustls::pki_types::CertificateDer,
) -> Result<Ed25519PublicKey, rustls::Error> {
    use x509_parser::{certificate::X509Certificate, prelude::FromDer};

    let cert = X509Certificate::from_der(certificate.as_ref())
        .map_err(|e| rustls::Error::General(e.to_string()))?;
    let spki = cert.1.public_key();
    let public_key_bytes =
        <ed25519::pkcs8::PublicKeyBytes as pkcs8::DecodePublicKey>::from_public_key_der(spki.raw)
            .map_err(|e| rustls::Error::General(format!("invalid ed25519 public key: {e}")))?;

    let public_key = Ed25519PublicKey::from_bytes(public_key_bytes.as_ref())
        .map_err(|e| rustls::Error::General(format!("invalid ed25519 public key: {e}")))?;
    Ok(public_key)
}

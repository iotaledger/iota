// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The functionality in this file is taken from https://docs.rs/axum-server/0.6.0/axum_server/tls_rustls/struct.RustlsAcceptor.html
//! because axum-server's rustls dependency is very out of date, and enabling
//! the `tsl-rustls` feature causes conflicts in our own usage of that lib.

use std::{fmt, io, sync::Arc, time::Duration};

use arc_swap::ArcSwap;
use axum::{middleware::AddExtension, Extension};
use axum_server::accept::{Accept, DefaultAcceptor};
use fastcrypto::ed25519::Ed25519PublicKey;
use rustls::{pki_types::CertificateDer, ServerConfig};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::server::TlsStream;
use tower_layer::Layer;

use self::future::RustlsAcceptorFuture;

#[derive(Debug, Clone)]
pub struct TlsConnectionInfo {
    sni_hostname: Option<Arc<str>>,
    peer_certificates: Option<Arc<[CertificateDer<'static>]>>,
    public_key: Option<Ed25519PublicKey>,
}

impl TlsConnectionInfo {
    pub fn sni_hostname(&self) -> Option<&str> {
        self.sni_hostname.as_deref()
    }

    pub fn peer_certificates(&self) -> Option<&[CertificateDer]> {
        self.peer_certificates.as_deref()
    }

    pub fn public_key(&self) -> Option<&Ed25519PublicKey> {
        self.public_key.as_ref()
    }
}

/// Rustls configuration.
#[derive(Clone)]
pub struct RustlsConfig {
    inner: Arc<ArcSwap<ServerConfig>>,
}

impl RustlsConfig {
    /// Create config from `Arc<`[`ServerConfig`]`>`.
    ///
    /// NOTE: You need to set ALPN protocols (like `http/1.1` or `h2`) manually.
    pub fn from_config(config: Arc<ServerConfig>) -> Self {
        let inner = Arc::new(ArcSwap::new(config));

        Self { inner }
    }

    /// Get  inner `Arc<`[`ServerConfig`]`>`.
    pub fn get_inner(&self) -> Arc<ServerConfig> {
        self.inner.load_full()
    }
}

impl fmt::Debug for RustlsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RustlsConfig").finish()
    }
}

/// An `Acceptor` that will provide `TlsConnectionInfo` as an axum `Extension`
/// for use in handlers.
#[derive(Debug, Clone)]
pub struct TlsAcceptor {
    inner: DefaultAcceptor,
    config: RustlsConfig,
    handshake_timeout: Duration,
}

impl TlsAcceptor {
    pub fn new(config: rustls::ServerConfig) -> Self {
        let inner = DefaultAcceptor::new();

        #[cfg(not(test))]
        let handshake_timeout = Duration::from_secs(10);

        // Don't force tests to wait too long.
        #[cfg(test)]
        let handshake_timeout = Duration::from_secs(1);

        Self {
            inner,
            config: RustlsConfig::from_config(Arc::new(config)),
            handshake_timeout,
        }
    }
}

type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

impl<I, S> Accept<I, S> for TlsAcceptor
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    S: Send + 'static,
{
    type Stream = TlsStream<I>;
    type Service = AddExtension<S, TlsConnectionInfo>;
    type Future = BoxFuture<'static, io::Result<(Self::Stream, Self::Service)>>;

    fn accept(&self, stream: I, service: S) -> Self::Future {
        let acceptor = self.inner;
        let config = self.config.clone();
        let handshake_timeout = self.handshake_timeout;

        Box::pin(async move {
            let inner_future = acceptor.accept(stream, service);

            let (stream, service) =
                RustlsAcceptorFuture::new(inner_future, config, handshake_timeout).await?;

            let server_conn = stream.get_ref().1;

            let public_key = if let Some([peer_certificate, ..]) = server_conn.peer_certificates() {
                crate::certgen::public_key_from_certificate(peer_certificate).ok()
            } else {
                None
            };

            let tls_connect_info = TlsConnectionInfo {
                peer_certificates: server_conn.peer_certificates().map(From::from),
                sni_hostname: server_conn.server_name().map(From::from),
                public_key,
            };
            let service = Extension(tls_connect_info).layer(service);

            Ok((stream, service))
        })
    }
}

pub mod future {
    use std::{
        fmt,
        future::Future,
        io,
        io::{Error, ErrorKind},
        pin::Pin,
        task::{Context, Poll},
        time::Duration,
    };

    use pin_project_lite::pin_project;
    use tokio::{
        io::{AsyncRead, AsyncWrite},
        time::{timeout, Timeout},
    };
    use tokio_rustls::{server::TlsStream, Accept, TlsAcceptor};

    use super::RustlsConfig;

    pin_project! {
        /// Future type for [`RustlsAcceptor`](crate::tls_rustls::RustlsAcceptor).
        pub struct RustlsAcceptorFuture<F, I, S> {
            #[pin]
            inner: AcceptFuture<F, I, S>,
            config: Option<RustlsConfig>,
        }
    }

    impl<F, I, S> RustlsAcceptorFuture<F, I, S> {
        pub(crate) fn new(future: F, config: RustlsConfig, handshake_timeout: Duration) -> Self {
            let inner = AcceptFuture::Inner {
                future,
                handshake_timeout,
            };
            let config = Some(config);

            Self { inner, config }
        }
    }

    impl<F, I, S> fmt::Debug for RustlsAcceptorFuture<F, I, S> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("RustlsAcceptorFuture").finish()
        }
    }

    pin_project! {
        #[project = AcceptFutureProj]
        enum AcceptFuture<F, I, S> {
            Inner {
                #[pin]
                future: F,
                handshake_timeout: Duration,
            },
            Accept {
                #[pin]
                future: Timeout<Accept<I>>,
                service: Option<S>,
            },
        }
    }

    impl<F, I, S> Future for RustlsAcceptorFuture<F, I, S>
    where
        F: Future<Output = io::Result<(I, S)>>,
        I: AsyncRead + AsyncWrite + Unpin,
    {
        type Output = io::Result<(TlsStream<I>, S)>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut this = self.project();

            loop {
                match this.inner.as_mut().project() {
                    AcceptFutureProj::Inner {
                        future,
                        handshake_timeout,
                    } => match future.poll(cx) {
                        Poll::Ready(Ok((stream, service))) => {
                            let server_config = this.config
                                .take()
                                .expect("config is not set. this is a bug in axum-server, please report")
                                .get_inner();

                            let acceptor = TlsAcceptor::from(server_config);
                            let future = acceptor.accept(stream);

                            let service = Some(service);
                            let handshake_timeout = *handshake_timeout;

                            this.inner.set(AcceptFuture::Accept {
                                future: timeout(handshake_timeout, future),
                                service,
                            });
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    },
                    AcceptFutureProj::Accept { future, service } => match future.poll(cx) {
                        Poll::Ready(Ok(Ok(stream))) => {
                            let service = service.take().expect("future polled after ready");

                            return Poll::Ready(Ok((stream, service)));
                        }
                        Poll::Ready(Ok(Err(e))) => return Poll::Ready(Err(e)),
                        Poll::Ready(Err(timeout)) => {
                            return Poll::Ready(Err(Error::new(ErrorKind::TimedOut, timeout)));
                        }
                        Poll::Pending => return Poll::Pending,
                    },
                }
            }
        }
    }
}

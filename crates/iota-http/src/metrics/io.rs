// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    io::{self, IoSlice},
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::metrics::{ConnectionGuard, ListenerMetrics};

/// IO wrapper made at accept. Its drop is the close of the connection.
pub(crate) struct TrackedIo<IO> {
    io: IO,
    connection: Option<ConnectionGuard>,
}

impl<IO> TrackedIo<IO> {
    /// Wraps `io`, a socket accepted from `remote`. With `metrics`, counts the
    /// connection. Without, every call passes through to `io`.
    pub(crate) fn new(io: IO, metrics: Option<&ListenerMetrics>, remote: SocketAddr) -> Self {
        Self {
            io,
            connection: metrics.map(|metrics| metrics.record_accept(remote)),
        }
    }

    pub(crate) fn connection(&self) -> Option<&ConnectionGuard> {
        self.connection.as_ref()
    }
}

impl<IO: AsyncRead + Unpin> AsyncRead for TrackedIo<IO> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl<IO: AsyncWrite + Unpin> AsyncWrite for TrackedIo<IO> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.io.is_write_vectored()
    }
}

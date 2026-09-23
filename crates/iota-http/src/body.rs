// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use http_body_util::BodyExt;
use pin_project_lite::pin_project;

use crate::{BoxError, activity::RequestGuard};

pub type BoxBody = http_body_util::combinators::UnsyncBoxBody<Bytes, BoxError>;

pin_project! {
    /// A response body that keeps its connection counted as busy until the
    /// body ends.
    ///
    /// The guard rides the body rather than the response future because a
    /// streaming response resolves its future as soon as the headers are
    /// ready, while the work continues for as long as frames are produced.
    pub(crate) struct GuardedBody<B> {
        #[pin]
        inner: B,
        guard: RequestGuard,
    }
}

impl<B> GuardedBody<B> {
    pub(crate) fn new(inner: B, guard: RequestGuard) -> Self {
        Self { inner, guard }
    }
}

impl<B: http_body::Body> http_body::Body for GuardedBody<B> {
    type Data = B::Data;
    type Error = B::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        self.project().inner.poll_frame(cx)
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

pub fn boxed<B>(body: B) -> BoxBody
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
{
    try_downcast(body).unwrap_or_else(|body| body.map_err(Into::into).boxed_unsync())
}

pub(crate) fn try_downcast<T, K>(k: K) -> Result<T, K>
where
    T: 'static,
    K: Send + 'static,
{
    let mut k = Some(k);
    if let Some(k) = <dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k) {
        Ok(k.take().unwrap())
    } else {
        Err(k.unwrap())
    }
}

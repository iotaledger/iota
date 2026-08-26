// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{future::Future, sync::Arc};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use iota_config::object_storage_config::TRANSFER_STALL_TIMEOUT;
use object_store::{DynObjectStore, ObjectMeta, ObjectStore, ObjectStoreExt, path::Path};
pub mod http;
pub mod util;

#[async_trait]
pub trait ObjectStoreGetExt: std::fmt::Display + Send + Sync + 'static {
    /// Return the bytes at given path in object store
    async fn get_bytes(&self, src: &Path) -> Result<Bytes>;

    /// Like [`Self::get_bytes`], additionally invoking `on_bytes` with the
    /// size of each received chunk while the download is in flight, so
    /// callers can render live progress. The default implementation reports
    /// the full size in one call once the download completes; stores that
    /// stream their responses override it to report per chunk.
    async fn get_bytes_with_progress(
        &self,
        src: &Path,
        on_bytes: &(dyn Fn(u64) + Send + Sync),
    ) -> Result<Bytes> {
        let bytes = self.get_bytes(src).await?;
        on_bytes(bytes.len() as u64);
        Ok(bytes)
    }

    /// Return whether an object exists at the given path.
    async fn exists(&self, src: &Path) -> Result<bool>;

    /// Return the size in bytes of the object at the given path.
    async fn object_size(&self, src: &Path) -> Result<u64>;
}

/// Await `fut`, failing if it does not resolve within
/// [`TRANSFER_STALL_TIMEOUT`].
async fn with_stall_timeout<F: Future>(task_name: &str, src: &Path, fut: F) -> Result<F::Output> {
    tokio::time::timeout(TRANSFER_STALL_TIMEOUT, fut)
        .await
        .map_err(|_| {
            anyhow!("{task_name} for file {src} received nothing for {TRANSFER_STALL_TIMEOUT:?}")
        })
}

/// Collect a GET result's payload into contiguous bytes, invoking `on_bytes`
/// with each chunk's size as it is received.
pub(crate) async fn collect_get_result_with_progress(
    result: object_store::GetResult,
    src: &Path,
    on_bytes: &(dyn Fn(u64) + Send + Sync),
) -> Result<Bytes> {
    use futures::StreamExt;
    let mut buf = Vec::with_capacity(result.meta.size as usize);
    let mut stream = result.into_stream();
    // Fails once no chunk has arrived for `TRANSFER_STALL_TIMEOUT`.
    while let Some(chunk) = with_stall_timeout("GET result stream", src, stream.next()).await? {
        let chunk = chunk
            .map_err(|e| anyhow!("Failed to stream GET result for file {src} with error: {e:?}"))?;
        on_bytes(chunk.len() as u64);
        buf.extend_from_slice(&chunk);
    }
    Ok(buf.into())
}

macro_rules! as_ref_get_ext_impl {
    ($type:ty) => {
        #[async_trait]
        impl ObjectStoreGetExt for $type {
            async fn get_bytes(&self, src: &Path) -> Result<Bytes> {
                self.as_ref().get_bytes(src).await
            }

            async fn get_bytes_with_progress(
                &self,
                src: &Path,
                on_bytes: &(dyn Fn(u64) + Send + Sync),
            ) -> Result<Bytes> {
                self.as_ref().get_bytes_with_progress(src, on_bytes).await
            }

            async fn exists(&self, src: &Path) -> Result<bool> {
                self.as_ref().exists(src).await
            }

            async fn object_size(&self, src: &Path) -> Result<u64> {
                self.as_ref().object_size(src).await
            }
        }
    };
}

as_ref_get_ext_impl!(Arc<dyn ObjectStoreGetExt>);
as_ref_get_ext_impl!(Box<dyn ObjectStoreGetExt>);

macro_rules! as_ref_get_impl {
    ($type:ty) => {
        #[async_trait]
        impl ObjectStoreGetExt for $type {
            async fn get_bytes(&self, src: &Path) -> Result<Bytes> {
                // Collected chunk by chunk rather than with `bytes()` so that a
                // transfer that stalls part way through hits the stall timeout.
                self.get_bytes_with_progress(src, &|_| {}).await
            }

            async fn get_bytes_with_progress(
                &self,
                src: &Path,
                on_bytes: &(dyn Fn(u64) + Send + Sync),
            ) -> Result<Bytes> {
                let result = with_stall_timeout("GET request", src, self.get(src))
                    .await?
                    .map_err(|e| anyhow!("Failed to get file {src} with error: {e:?}"))?;
                collect_get_result_with_progress(result, src, on_bytes).await
            }

            async fn exists(&self, src: &Path) -> Result<bool> {
                match with_stall_timeout("HEAD request", src, self.head(src)).await? {
                    Ok(_) => Ok(true),
                    Err(object_store::Error::NotFound { .. }) => Ok(false),
                    Err(e) => Err(anyhow!(
                        "Failed to check if file {src} exists with error: {e:?}"
                    )),
                }
            }

            async fn object_size(&self, src: &Path) -> Result<u64> {
                with_stall_timeout("HEAD request", src, self.head(src))
                    .await?
                    .map(|meta| meta.size)
                    .map_err(|e| anyhow!("Failed to get size of file {src} with error: {e:?}"))
            }
        }
    };
}

as_ref_get_impl!(Arc<dyn ObjectStore>);
as_ref_get_impl!(Box<dyn ObjectStore>);

#[async_trait]
pub trait ObjectStoreListExt: Send + Sync + 'static {
    /// List the objects at the given path in object store
    async fn list_objects(
        &self,
        src: Option<&Path>,
    ) -> BoxStream<'_, object_store::Result<ObjectMeta>>;
}

macro_rules! as_ref_list_ext_impl {
    ($type:ty) => {
        #[async_trait]
        impl ObjectStoreListExt for $type {
            async fn list_objects(
                &self,
                src: Option<&Path>,
            ) -> BoxStream<'_, object_store::Result<ObjectMeta>> {
                self.as_ref().list_objects(src).await
            }
        }
    };
}

as_ref_list_ext_impl!(Arc<dyn ObjectStoreListExt>);
as_ref_list_ext_impl!(Box<dyn ObjectStoreListExt>);

#[async_trait]
impl ObjectStoreListExt for Arc<DynObjectStore> {
    async fn list_objects(
        &self,
        src: Option<&Path>,
    ) -> BoxStream<'_, object_store::Result<ObjectMeta>> {
        self.list(src)
    }
}

#[async_trait]
pub trait ObjectStorePutExt: Send + Sync + 'static {
    /// Write the bytes at the given location in object store
    async fn put_bytes(&self, src: &Path, bytes: Bytes) -> Result<()>;
}

macro_rules! as_ref_put_ext_impl {
    ($type:ty) => {
        #[async_trait]
        impl ObjectStorePutExt for $type {
            async fn put_bytes(&self, src: &Path, bytes: Bytes) -> Result<()> {
                self.as_ref().put_bytes(src, bytes).await
            }
        }
    };
}

as_ref_put_ext_impl!(Arc<dyn ObjectStorePutExt>);
as_ref_put_ext_impl!(Box<dyn ObjectStorePutExt>);

#[async_trait]
impl ObjectStorePutExt for Arc<DynObjectStore> {
    async fn put_bytes(&self, src: &Path, bytes: Bytes) -> Result<()> {
        self.put(src, bytes.into()).await?;
        Ok(())
    }
}

#[async_trait]
pub trait ObjectStoreDeleteExt: Send + Sync + 'static {
    /// Delete the object at the given location in object store
    async fn delete_object(&self, src: &Path) -> Result<()>;
}

macro_rules! as_ref_delete_ext_impl {
    ($type:ty) => {
        #[async_trait]
        impl ObjectStoreDeleteExt for $type {
            async fn delete_object(&self, src: &Path) -> Result<()> {
                self.as_ref().delete_object(src).await
            }
        }
    };
}

as_ref_delete_ext_impl!(Arc<dyn ObjectStoreDeleteExt>);
as_ref_delete_ext_impl!(Box<dyn ObjectStoreDeleteExt>);

#[async_trait]

impl ObjectStoreDeleteExt for Arc<DynObjectStore> {
    async fn delete_object(&self, src: &Path) -> Result<()> {
        self.delete(src).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    use bytes::Bytes;
    use futures::StreamExt;
    use object_store::{
        Attributes, GetResult, GetResultPayload, ObjectMeta, ObjectStore, memory::InMemory,
        path::Path,
    };

    use crate::object_store::{
        ObjectStoreGetExt, ObjectStorePutExt, collect_get_result_with_progress,
    };

    #[tokio::test]
    async fn test_dyn_object_store_get_bytes() -> anyhow::Result<()> {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let path = Path::from("file1");
        store
            .put_bytes(&path, Bytes::from_static(b"Lorem ipsum"))
            .await?;

        assert_eq!(
            store.get_bytes(&path).await?,
            Bytes::from_static(b"Lorem ipsum")
        );
        assert!(store.get_bytes(&Path::from("missing")).await.is_err());
        Ok(())
    }

    /// A transfer that goes quiet part way through fails rather than hanging.
    /// Runs on a paused clock, so the stall timeout elapses without the test
    /// waiting for it.
    #[tokio::test(start_paused = true)]
    async fn test_collect_get_result_with_progress_fails_on_a_stalled_stream() {
        let src = Path::from("file1");
        let payload = futures::stream::once(async {
            Ok::<_, object_store::Error>(Bytes::from_static(b"Lorem"))
        })
        .chain(futures::stream::pending())
        .boxed();
        let result = GetResult {
            range: 0..11,
            payload: GetResultPayload::Stream(payload),
            meta: ObjectMeta {
                location: src.clone(),
                last_modified: chrono::Utc::now(),
                size: 11,
                e_tag: None,
                version: None,
            },
            attributes: Attributes::new(),
        };

        let received = AtomicU64::new(0);
        let err = collect_get_result_with_progress(result, &src, &|n| {
            received.fetch_add(n, Ordering::Relaxed);
        })
        .await
        .unwrap_err();

        assert!(err.to_string().contains("received nothing"), "{err}");
        assert_eq!(received.load(Ordering::Relaxed), 5);
    }

    #[tokio::test]
    async fn test_dyn_object_store_exists() -> anyhow::Result<()> {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let path = Path::from("file1");
        store
            .put_bytes(&path, bytes::Bytes::from_static(b"Lorem ipsum"))
            .await?;

        assert!(store.exists(&path).await?);
        assert!(!store.exists(&Path::from("missing")).await?);
        Ok(())
    }

    #[tokio::test]
    async fn test_dyn_object_store_object_size() -> anyhow::Result<()> {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let path = Path::from("file1");
        store
            .put_bytes(&path, bytes::Bytes::from_static(b"Lorem ipsum"))
            .await?;

        assert_eq!(store.object_size(&path).await?, 11);
        assert!(store.object_size(&Path::from("missing")).await.is_err());
        Ok(())
    }
}

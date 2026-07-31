// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Progress reporting for snapshot downloads.
//!
//! When the total byte size of a download is known — from a size sweep over
//! the remote files with [`fetch_total_bytes`] — the bar is byte-denominated
//! and indicatif renders the size totals, download speed, and ETA natively,
//! while the file counts move to the bar message. When the sweep fails, the
//! bar falls back to file counts.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use backoff::future::retry;
use bytes::Bytes;
use futures::{StreamExt, TryStreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use iota_storage::object_store::{ObjectStoreGetExt, ObjectStorePutExt, util::put};
use object_store::path::Path;
use tracing::{error, warn};

/// Fetch the combined size in bytes of every path in `paths` from the remote
/// store with bounded concurrency, rendering a progress bar on `m` while the
/// lookups run. Returns `None` after logging a warning if any lookup fails —
/// a partial sum would misstate the total: progress reporting is best-effort
/// and must never fail a download.
pub async fn fetch_total_bytes(
    remote_object_store: &Arc<dyn ObjectStoreGetExt>,
    paths: Vec<Path>,
    concurrency: usize,
    m: &MultiProgress,
) -> Option<u64> {
    let bar = m.add(
        ProgressBar::new(paths.len() as u64).with_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {wide_bar} Fetching file sizes: {pos}/{len} (ETA {eta})",
            )
            .unwrap(),
        ),
    );
    bar.enable_steady_tick(Duration::from_millis(100));
    let mut stream = futures::stream::iter(paths)
        .map(|path| {
            let store = remote_object_store.clone();
            let bar = bar.clone();
            async move {
                // Sizes are best-effort: retry briefly so a single transient
                // error out of thousands of lookups doesn't hide the totals,
                // but give up quickly rather than stall the restore.
                let size = retry(size_lookup_backoff(), || async {
                    store
                        .object_size(&path)
                        .await
                        .map_err(backoff::Error::transient)
                })
                .await;
                bar.inc(1);
                (path, size)
            }
        })
        .buffer_unordered(concurrency);
    let mut total_bytes = 0u64;
    while let Some((path, size)) = stream.next().await {
        match size {
            Ok(size) => total_bytes += size,
            Err(err) => {
                warn!("size lookup for {path} failed, download totals will not be shown: {err:?}");
                // Dropping the stream cancels the remaining lookups rather
                // than waiting for every one to finish first.
                bar.finish_and_clear();
                return None;
            }
        }
    }
    bar.finish_and_clear();
    Some(total_bytes)
}

fn size_lookup_backoff() -> backoff::ExponentialBackoff {
    backoff::ExponentialBackoff {
        max_elapsed_time: Some(Duration::from_secs(3)),
        ..Default::default()
    }
}

/// A download progress bar added to `m`: byte-denominated when `total_bytes`
/// is known (indicatif renders the size totals, download speed, and ETA; the
/// file counts go to the message), file-count based otherwise.
///
/// `phase` is rendered verbatim at the start of the bar line, e.g.
/// "Downloading files".
pub struct DownloadProgressBar {
    bar: ProgressBar,
    num_files: u64,
    files_done: AtomicU64,
    byte_denominated: bool,
}

impl DownloadProgressBar {
    pub fn new(m: &MultiProgress, phase: &str, num_files: u64, total_bytes: Option<u64>) -> Self {
        let bar = match total_bytes {
            Some(total_bytes) => ProgressBar::new(total_bytes).with_style(
                ProgressStyle::with_template(&format!(
                    "[{{elapsed_precise}}] {{wide_bar}} {phase}: \
                     {{binary_bytes}}/{{binary_total_bytes}} ({{binary_bytes_per_sec}}, \
                     ETA {{eta}}) — {{msg}}"
                ))
                .unwrap(),
            ),
            None => ProgressBar::new(num_files).with_style(
                ProgressStyle::with_template(&format!(
                    "[{{elapsed_precise}}] {{wide_bar}} {phase}: {{pos}}/{{len}} files \
                     (ETA {{eta}}) — {{msg}}"
                ))
                .unwrap(),
            ),
        };
        let bar = m.add(bar);
        // Render the bar on a timer so it stays visible while the first files
        // are still in flight, rather than only repainting on completion.
        bar.enable_steady_tick(Duration::from_millis(100));
        Self {
            bar,
            num_files,
            files_done: AtomicU64::new(0),
            byte_denominated: total_bytes.is_some(),
        }
    }

    /// Record `n` freshly received bytes of an in-flight download.
    pub fn add_bytes(&self, n: u64) {
        if self.byte_denominated {
            self.bar.inc(n);
        }
    }

    /// Roll back bytes recorded by a failed download attempt, so the retry
    /// doesn't count them twice.
    pub fn remove_bytes(&self, n: u64) {
        if self.byte_denominated {
            // The read-modify-write races with concurrent `add_bytes` calls
            // from other in-flight downloads (indicatif has no atomic
            // decrement); a lost increment only skews the displayed position
            // slightly, which is acceptable for a progress bar.
            self.bar.set_position(self.bar.position().saturating_sub(n));
        }
    }

    /// Record one fully downloaded file, whose bytes were already recorded
    /// with [`Self::add_bytes`] as they arrived.
    pub fn file_done(&self) {
        let files_done = self.files_done.fetch_add(1, Ordering::Relaxed) + 1;
        if self.byte_denominated {
            self.bar
                .set_message(format!("{files_done}/{} files", self.num_files));
        } else {
            self.bar.inc(1);
        }
    }

    /// The underlying bar, e.g. to finish it with a message.
    pub fn bar(&self) -> &ProgressBar {
        &self.bar
    }
}

/// Download `src` from `store` with retries, recording received chunks on
/// `progress` as they arrive. A failed attempt's bytes are rolled back so the
/// retry doesn't count them twice.
pub async fn get_with_progress<S: ObjectStoreGetExt>(
    store: &S,
    src: &Path,
    progress: &DownloadProgressBar,
) -> Result<Bytes> {
    retry(backoff::ExponentialBackoff::default(), || async {
        let attempt_bytes = AtomicU64::new(0);
        store
            .get_bytes_with_progress(src, &|n| {
                attempt_bytes.fetch_add(n, Ordering::Relaxed);
                progress.add_bytes(n);
            })
            .await
            .map_err(|e| {
                progress.remove_bytes(attempt_bytes.load(Ordering::Relaxed));
                error!("Failed to read file {src} from object store with error: {e:?}");
                backoff::Error::transient(e)
            })
    })
    .await
}

/// Copy `src[i]` to `dest[i]` for all `i` in parallel, recording downloaded
/// chunks and each completed file on `progress` and failing on the first copy
/// that fails.
pub async fn copy_files_with_progress<S: ObjectStoreGetExt, D: ObjectStorePutExt>(
    src: &[Path],
    dest: &[Path],
    src_store: &S,
    dest_store: &D,
    concurrency: usize,
    progress: &DownloadProgressBar,
) -> Result<()> {
    futures::stream::iter(src.iter().zip(dest.iter()))
        .map(|(path_in, path_out)| async move {
            let bytes = get_with_progress(src_store, path_in, progress).await?;
            put(dest_store, path_out, bytes).await?;
            Ok::<_, anyhow::Error>(())
        })
        .boxed()
        .buffer_unordered(concurrency)
        .try_for_each(|()| {
            progress.file_done();
            futures::future::ready(Ok(()))
        })
        .await
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use anyhow::Result;
    use async_trait::async_trait;
    use bytes::Bytes;
    use indicatif::{MultiProgress, ProgressDrawTarget};
    use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
    use iota_storage::object_store::{ObjectStoreGetExt, ObjectStorePutExt};
    use object_store::{ObjectStore, memory::InMemory, path::Path};

    use super::{
        DownloadProgressBar, copy_files_with_progress, fetch_total_bytes, get_with_progress,
    };

    fn hidden_multi_progress() -> MultiProgress {
        MultiProgress::with_draw_target(ProgressDrawTarget::hidden())
    }

    #[tokio::test]
    async fn test_copy_files_with_progress_advances_by_bytes() -> anyhow::Result<()> {
        let src_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let dest_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        src_store
            .put_bytes(&Path::from("a"), Bytes::from_static(b"12345"))
            .await?;
        src_store
            .put_bytes(&Path::from("b"), Bytes::from_static(b"123"))
            .await?;
        let paths = [Path::from("a"), Path::from("b")];

        let progress =
            DownloadProgressBar::new(&hidden_multi_progress(), "Downloading", 2, Some(8));
        copy_files_with_progress(&paths, &paths, &src_store, &dest_store, 2, &progress).await?;
        assert_eq!(progress.bar().length(), Some(8));
        assert_eq!(progress.bar().position(), 8);
        assert_eq!(progress.bar().message(), "2/2 files");
        assert_eq!(
            dest_store.get_bytes(&Path::from("a")).await?.to_vec(),
            b"12345"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_copy_files_with_progress_falls_back_to_file_counts() -> anyhow::Result<()> {
        let src_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let dest_store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        src_store
            .put_bytes(&Path::from("a"), Bytes::from_static(b"12345"))
            .await?;
        src_store
            .put_bytes(&Path::from("b"), Bytes::from_static(b"123"))
            .await?;
        let paths = [Path::from("a"), Path::from("b")];

        // An unknown total byte size means the bar counts files instead.
        let progress = DownloadProgressBar::new(&hidden_multi_progress(), "Downloading", 2, None);
        copy_files_with_progress(&paths, &paths, &src_store, &dest_store, 2, &progress).await?;
        assert_eq!(progress.bar().length(), Some(2));
        assert_eq!(progress.bar().position(), 2);
        Ok(())
    }

    /// Store whose first download attempt emits a partial chunk and then
    /// fails; every later attempt succeeds.
    struct FlakyStore {
        payload: Bytes,
        failed_once: AtomicBool,
    }

    impl std::fmt::Display for FlakyStore {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "flaky")
        }
    }

    #[async_trait]
    impl ObjectStoreGetExt for FlakyStore {
        async fn get_bytes(&self, src: &Path) -> Result<Bytes> {
            self.get_bytes_with_progress(src, &|_| {}).await
        }

        async fn get_bytes_with_progress(
            &self,
            _src: &Path,
            on_bytes: &(dyn Fn(u64) + Send + Sync),
        ) -> Result<Bytes> {
            if !self.failed_once.swap(true, Ordering::Relaxed) {
                on_bytes(3);
                anyhow::bail!("transient failure after a partial chunk");
            }
            on_bytes(self.payload.len() as u64);
            Ok(self.payload.clone())
        }

        async fn exists(&self, _src: &Path) -> Result<bool> {
            Ok(true)
        }

        async fn object_size(&self, _src: &Path) -> Result<u64> {
            Ok(self.payload.len() as u64)
        }
    }

    /// A failed attempt's partially counted bytes are rolled back, so the
    /// retry's bytes aren't counted on top of them.
    #[tokio::test]
    async fn test_get_with_progress_rolls_back_failed_attempt() -> Result<()> {
        let store = FlakyStore {
            payload: Bytes::from_static(b"12345"),
            failed_once: AtomicBool::new(false),
        };
        let progress =
            DownloadProgressBar::new(&hidden_multi_progress(), "Downloading", 1, Some(5));

        let bytes = get_with_progress(&store, &Path::from("a"), &progress).await?;
        assert_eq!(bytes.to_vec(), b"12345");
        assert_eq!(progress.bar().position(), 5);
        Ok(())
    }

    /// A failed size lookup for any path in the batch means the download
    /// totals will not be shown, rather than reporting a partial sum.
    #[tokio::test]
    async fn test_fetch_total_bytes_is_all_or_nothing() -> anyhow::Result<()> {
        let tmp_dir = tempfile::tempdir()?;
        std::fs::write(tmp_dir.path().join("present"), b"hello")?;
        std::fs::write(tmp_dir.path().join("other"), b"abc")?;
        let store_config = ObjectStoreConfig {
            object_store: Some(ObjectStoreType::File),
            directory: Some(tmp_dir.path().to_path_buf()),
            ..Default::default()
        };
        let store: Arc<dyn ObjectStoreGetExt> = Arc::new(store_config.make()?);

        let total = fetch_total_bytes(
            &store,
            vec![Path::from("present"), Path::from("other")],
            2,
            &hidden_multi_progress(),
        )
        .await;
        assert_eq!(total, Some(8));

        // Any failed lookup disables the displayed download totals entirely.
        let total = fetch_total_bytes(
            &store,
            vec![Path::from("present"), Path::from("missing")],
            2,
            &hidden_multi_progress(),
        )
        .await;
        assert!(total.is_none());
        Ok(())
    }
}

// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Progress reporting for snapshot downloads.
//!
//! When the total byte size of a download is known — from a size sweep over
//! the remote files with [`fetch_total_bytes`] — the bar is byte-denominated
//! and indicatif renders the size totals, download speed, and ETA natively,
//! while the file counts move to the bar message. When the sweep fails, the
//! bar falls back to file counts.
//!
//! Bars can only be drawn on a terminal. Whenever they can't be — output is
//! piped or redirected, or `--disable-progress-bar` was passed — every bar's
//! [`ProgressTicker`] logs the same information as a status line once per
//! second instead.

use std::{
    borrow::Cow,
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
use indicatif::{
    FormattedDuration, HumanBytes, HumanCount, HumanDuration, MultiProgress, ProgressBar,
    ProgressDrawTarget, ProgressStyle,
};
use iota_storage::object_store::{ObjectStoreGetExt, ObjectStorePutExt, util::put};
use object_store::path::Path;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// The `tracing` target every status line is logged under, so that a caller
/// which otherwise silences its logs can keep them.
pub const LOG_TARGET_PROGRESS: &str = module_path!();

/// Report `msg` on the progress display, or log it when the display is hidden
/// — [`MultiProgress::println`] writes nothing at all there, so the phase
/// announcements around the bars would otherwise be lost in exactly the runs
/// that depend on the status lines.
pub fn println_or_log(m: &MultiProgress, msg: impl AsRef<str>) -> std::io::Result<()> {
    if m.is_hidden() {
        info!(target: LOG_TARGET_PROGRESS, "{}", msg.as_ref());
        Ok(())
    } else {
        m.println(msg)
    }
}

/// The progress display for a restore: one that draws its bars on the
/// terminal, or a hidden one — whose bars report through periodic status logs
/// instead — when `disable_progress_bar` is set.
///
/// A display bound to a non-terminal output stream is hidden either way, so
/// callers get the status logs there without passing the flag.
pub fn make_multi_progress(disable_progress_bar: bool) -> MultiProgress {
    if disable_progress_bar {
        MultiProgress::with_draw_target(ProgressDrawTarget::hidden())
    } else {
        MultiProgress::new()
    }
}

/// What a progress bar's position counts, used to render its status lines.
#[derive(Clone, Copy)]
pub enum ProgressUnit {
    /// The position is a number of bytes.
    Bytes,
    /// The position counts items named by the given plural noun, e.g. "files".
    Count(&'static str),
}

/// Reports a progress bar's state once per second for as long as it runs:
/// mirrors `counter` into the bar's position if one was given, and logs a
/// status line carrying the same totals, rate and ETA the bar shows whenever
/// the bar itself can't be drawn.
///
/// Finish the bar through [`Self::finish_with_message`] or
/// [`Self::finish_and_clear`] rather than on the bar itself, so the final
/// status line is logged even when the bar finishes before the first second is
/// up. Dropping the ticker stops the reporting.
pub struct ProgressTicker {
    bar: ProgressBar,
    phase: &'static str,
    unit: ProgressUnit,
    task: Option<JoinHandle<()>>,
}

impl ProgressTicker {
    /// Starts reporting `bar`, whose position counts `unit` and whose progress
    /// is logged under `phase`, e.g. "Downloading files".
    ///
    /// When a `counter` is given, the bar's position is taken from it once per
    /// second rather than being advanced by the caller.
    pub fn spawn(
        bar: ProgressBar,
        phase: &'static str,
        unit: ProgressUnit,
        counter: Option<Arc<AtomicU64>>,
    ) -> Self {
        // A drawn bar renders its own totals, so there is only something to do
        // once per second if the bar is hidden or its position has to be
        // mirrored from a counter.
        let log_status = bar.is_hidden();
        let task = (log_status || counter.is_some()).then(|| {
            let bar = bar.clone();
            tokio::spawn(async move {
                while !bar.is_finished() {
                    if let Some(counter) = &counter {
                        bar.set_position(counter.load(Ordering::Relaxed));
                    }
                    if log_status {
                        info!(
                            target: LOG_TARGET_PROGRESS,
                            "{}",
                            status_line(&bar, phase, &unit, None)
                        );
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            })
        });
        Self {
            bar,
            phase,
            unit,
            task,
        }
    }

    /// Finishes the bar with `msg`, leaving it on screen.
    pub fn finish_with_message(&self, msg: &'static str) {
        self.finish(msg, false);
    }

    /// Finishes the bar with `msg` and removes it from the display, for a
    /// phase whose completed bar isn't worth keeping.
    pub fn finish_and_clear(&self, msg: &'static str) {
        self.finish(msg, true);
    }

    /// The underlying bar, to advance it or set its message.
    pub fn bar(&self) -> &ProgressBar {
        &self.bar
    }

    fn finish(&self, msg: &'static str, clear: bool) {
        // Stop the ticker first so it can't log a periodic line after the
        // final one.
        self.stop();
        if clear {
            self.bar.finish_and_clear();
        } else {
            self.bar.finish_with_message(msg);
        }
        if self.bar.is_hidden() {
            info!(
                target: LOG_TARGET_PROGRESS,
                "{}",
                status_line(&self.bar, self.phase, &self.unit, Some(msg))
            );
        }
    }

    fn stop(&self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

impl Drop for ProgressTicker {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Renders `bar`'s state as one log line: elapsed time, phase, how far it has
/// got, its rate and ETA, and finally either `done_msg` or the bar's own
/// message.
///
/// The rate and ETA are left out until the bar has moved far enough to
/// estimate them, and the ETA also once the bar has finished.
fn status_line(
    bar: &ProgressBar,
    phase: &str,
    unit: &ProgressUnit,
    done_msg: Option<&str>,
) -> String {
    let pos = bar.position();
    // Every snapshot bar is created with a length; falling back to the
    // position keeps a lengthless one from reporting a total of zero.
    let len = bar.length().unwrap_or(pos);
    let progress = match unit {
        ProgressUnit::Bytes => format!("{}/{}", HumanBytes(pos), HumanBytes(len)),
        ProgressUnit::Count(unit) => format!("{}/{} {unit}", HumanCount(pos), HumanCount(len)),
    };
    let mut line = format!("[{}] {phase}: {progress}", FormattedDuration(bar.elapsed()));
    let rate = bar.per_sec();
    if rate > 0.0 {
        let rate = match unit {
            ProgressUnit::Bytes => format!("{}/s", HumanBytes(rate as u64)),
            ProgressUnit::Count(unit) => format!("{rate:.1} {unit}/s"),
        };
        if bar.is_finished() {
            line.push_str(&format!(" ({rate})"));
        } else {
            line.push_str(&format!(" ({rate}, ETA {})", HumanDuration(bar.eta())));
        }
    }
    let msg: Cow<'_, str> = done_msg.map_or_else(|| bar.message().into(), Into::into);
    if !msg.is_empty() {
        line.push_str(&format!(" — {msg}"));
    }
    line
}

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
    steady_tick_when_drawn(&bar);
    let ticker = ProgressTicker::spawn(
        bar.clone(),
        "Fetching file sizes",
        ProgressUnit::Count("files"),
        None,
    );
    let mut stream = futures::stream::iter(paths)
        .map(|path| {
            let store = remote_object_store.clone();
            let bar = bar.clone();
            async move {
                // Sizes are best-effort: retry briefly so a single transient
                // error out of thousands of lookups doesn't hide the totals,
                // but give up quickly rather than stall the restore.
                let size = retry(size_lookup_backoff(), || async {
                    store.object_size(&path).await.map_err(classify_for_retry)
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
                ticker.finish_and_clear("Size lookup failed");
                return None;
            }
        }
    }
    ticker.finish_and_clear("File sizes fetched");
    Some(total_bytes)
}

/// Repaints `bar` on a timer so it stays current while its work is in flight,
/// rather than only when the caller advances it. A hidden bar is never
/// repainted; its [`ProgressTicker`] reports it instead.
fn steady_tick_when_drawn(bar: &ProgressBar) {
    if !bar.is_hidden() {
        bar.enable_steady_tick(Duration::from_millis(100));
    }
}

/// Wrap `err` for [`retry`]: an error retrying cannot fix — a missing object
/// or denied access — gives up immediately, everything else is retried.
fn classify_for_retry(err: anyhow::Error) -> backoff::Error<anyhow::Error> {
    let is_permanent = err.chain().any(|cause| {
        cause
            .downcast_ref::<reqwest::Error>()
            .and_then(reqwest::Error::status)
            .is_some_and(|status| {
                matches!(
                    status,
                    reqwest::StatusCode::NOT_FOUND
                        | reqwest::StatusCode::FORBIDDEN
                        | reqwest::StatusCode::UNAUTHORIZED
                )
            })
    });

    if is_permanent {
        backoff::Error::permanent(err)
    } else {
        backoff::Error::transient(err)
    }
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
    ticker: ProgressTicker,
}

impl DownloadProgressBar {
    pub fn new(
        m: &MultiProgress,
        phase: &'static str,
        num_files: u64,
        total_bytes: Option<u64>,
    ) -> Self {
        // A single-file download has no file counts to carry alongside its
        // bytes, so it leaves the bar's message out entirely.
        let msg = if num_files > 1 { " — {msg}" } else { "" };
        let bar = match total_bytes {
            Some(total_bytes) => ProgressBar::new(total_bytes).with_style(
                ProgressStyle::with_template(&format!(
                    "[{{elapsed_precise}}] {{wide_bar}} {phase}: \
                     {{binary_bytes}}/{{binary_total_bytes}} ({{binary_bytes_per_sec}}, \
                     ETA {{eta}}){msg}"
                ))
                .unwrap(),
            ),
            None => ProgressBar::new(num_files).with_style(
                ProgressStyle::with_template(&format!(
                    "[{{elapsed_precise}}] {{wide_bar}} {phase}: {{pos}}/{{len}} files \
                     (ETA {{eta}}){msg}"
                ))
                .unwrap(),
            ),
        };
        let bar = m.add(bar);
        steady_tick_when_drawn(&bar);
        let unit = match total_bytes {
            Some(_) => ProgressUnit::Bytes,
            None => ProgressUnit::Count("files"),
        };
        Self {
            bar: bar.clone(),
            num_files,
            files_done: AtomicU64::new(0),
            byte_denominated: total_bytes.is_some(),
            ticker: ProgressTicker::spawn(bar, phase, unit, None),
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

    /// Records that both download phases are done, under the message `msg`.
    pub fn finish_with_message(&self, msg: &'static str) {
        self.ticker.finish_with_message(msg);
    }

    /// The underlying bar, e.g. to read its position in tests.
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
                classify_for_retry(e)
            })
    })
    .await
}

/// Download the single file `src` from `store` with retries, reporting the
/// bytes as they arrive on a bar of its own under `phase` — for a file big
/// enough that a restore shouldn't sit silent while it downloads.
pub async fn get_single_file_with_progress<S: ObjectStoreGetExt>(
    store: &S,
    src: &Path,
    phase: &'static str,
    m: &MultiProgress,
) -> Result<Bytes> {
    // The size only feeds the bar's totals, so a failed lookup costs the size
    // and ETA rather than the download.
    let total_bytes = store.object_size(src).await.ok();
    let progress = DownloadProgressBar::new(m, phase, 1, total_bytes);
    let bytes = get_with_progress(store, src, &progress).await?;
    progress.finish_with_message("Download complete");
    Ok(bytes)
}

/// Copy every path in `paths` from `src_store` to `dest_store` in parallel,
/// recording downloaded chunks and each completed file on `progress` and
/// failing on the first copy that fails.
pub async fn copy_files_with_progress<S: ObjectStoreGetExt, D: ObjectStorePutExt>(
    paths: &[Path],
    src_store: &S,
    dest_store: &D,
    concurrency: usize,
    progress: &DownloadProgressBar,
) -> Result<()> {
    futures::stream::iter(paths)
        .map(|path| async move {
            let bytes = get_with_progress(src_store, path, progress).await?;
            put(dest_store, path, bytes).await?;
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
    use std::{
        io,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        time::Duration,
    };

    use anyhow::Result;
    use async_trait::async_trait;
    use bytes::Bytes;
    use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, TermLike};
    use iota_config::object_storage_config::{ObjectStoreConfig, ObjectStoreType};
    use iota_storage::object_store::{ObjectStoreGetExt, ObjectStorePutExt};
    use object_store::{ObjectStore, memory::InMemory, path::Path};
    use tracing_subscriber::fmt::MakeWriter;

    use super::{
        DownloadProgressBar, ProgressTicker, ProgressUnit, copy_files_with_progress,
        fetch_total_bytes, get_with_progress, println_or_log, status_line,
    };

    fn hidden_multi_progress() -> MultiProgress {
        MultiProgress::with_draw_target(ProgressDrawTarget::hidden())
    }

    /// A terminal that accepts every write, so that bars added to the
    /// resulting display count as drawn rather than hidden.
    #[derive(Debug)]
    struct DrawnTerm;

    impl TermLike for DrawnTerm {
        fn width(&self) -> u16 {
            80
        }

        fn move_cursor_up(&self, _n: usize) -> io::Result<()> {
            Ok(())
        }

        fn move_cursor_down(&self, _n: usize) -> io::Result<()> {
            Ok(())
        }

        fn move_cursor_right(&self, _n: usize) -> io::Result<()> {
            Ok(())
        }

        fn move_cursor_left(&self, _n: usize) -> io::Result<()> {
            Ok(())
        }

        fn write_line(&self, _s: &str) -> io::Result<()> {
            Ok(())
        }

        fn write_str(&self, _s: &str) -> io::Result<()> {
            Ok(())
        }

        fn clear_line(&self) -> io::Result<()> {
            Ok(())
        }

        fn flush(&self) -> io::Result<()> {
            Ok(())
        }
    }

    fn drawn_multi_progress() -> MultiProgress {
        MultiProgress::with_draw_target(ProgressDrawTarget::term_like(Box::new(DrawnTerm)))
    }

    /// Everything logged by `f` on the calling thread.
    fn captured_logs(f: impl FnOnce()) -> String {
        #[derive(Clone)]
        struct Sink(Arc<Mutex<Vec<u8>>>);

        impl io::Write for Sink {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        impl MakeWriter<'_> for Sink {
            type Writer = Self;

            fn make_writer(&self) -> Self::Writer {
                self.clone()
            }
        }

        let buf = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(Sink(buf.clone()))
            .with_ansi(false)
            .without_time()
            .finish();
        tracing::subscriber::with_default(subscriber, f);
        let logs = buf.lock().unwrap().clone();
        String::from_utf8(logs).expect("log output is not valid UTF-8")
    }

    /// A status line carries the same totals, rate and ETA as the bar, and
    /// leaves the rate and ETA out until there is progress to estimate from.
    #[tokio::test]
    async fn test_status_line_reports_totals_rate_and_eta() {
        let bar = hidden_multi_progress().add(ProgressBar::new(100));

        let line = status_line(&bar, "Downloading files", &ProgressUnit::Bytes, None);
        assert!(line.ends_with("Downloading files: 0 B/100 B"), "{line}");

        bar.inc(50);
        bar.set_message("1/2 files");
        let line = status_line(&bar, "Downloading files", &ProgressUnit::Bytes, None);
        assert!(line.contains("Downloading files: 50 B/100 B ("), "{line}");
        assert!(line.contains("B/s, ETA "), "{line}");
        assert!(line.ends_with(" — 1/2 files"), "{line}");

        // A counting bar names what it counts instead.
        let line = status_line(
            &bar,
            "Checksumming ref files",
            &ProgressUnit::Count("ref files"),
            None,
        );
        assert!(
            line.contains("Checksumming ref files: 50/100 ref files ("),
            "{line}"
        );
        assert!(line.contains(" ref files/s, ETA "), "{line}");
    }

    /// The final line is logged whenever the phase ends, even if the bar
    /// finished before the first second was up, and it reports the finishing
    /// message rather than an ETA.
    #[tokio::test]
    async fn test_finish_logs_a_final_status_line_immediately() {
        let progress =
            DownloadProgressBar::new(&hidden_multi_progress(), "Downloading files", 2, Some(8));

        let logs = captured_logs(|| progress.finish_with_message("Download complete"));
        assert!(logs.contains("Downloading files: 8 B/8 B"), "{logs}");
        assert!(logs.ends_with("— Download complete\n"), "{logs}");
        assert!(!logs.contains("ETA"), "{logs}");
    }

    /// A hidden display drops everything handed to `MultiProgress::println`,
    /// so the phase announcements around the bars are logged there instead.
    #[tokio::test]
    async fn test_phase_announcements_are_logged_when_the_display_is_hidden() {
        let hidden = hidden_multi_progress();
        let logs = captured_logs(|| {
            println_or_log(&hidden, "Loading genesis from genesis.blob").expect("hidden println");
        });
        assert!(logs.contains("Loading genesis from genesis.blob"), "{logs}");

        // A drawn display prints them itself, with no log line.
        let drawn = drawn_multi_progress();
        let logs = captured_logs(|| {
            println_or_log(&drawn, "Loading genesis from genesis.blob").expect("drawn println");
        });
        assert!(logs.is_empty(), "{logs}");
    }

    /// A bar the terminal can draw reports through the display, so it logs no
    /// status lines at all.
    #[tokio::test]
    async fn test_a_drawn_bar_logs_no_status_lines() {
        let bar = drawn_multi_progress().add(ProgressBar::new(1));
        assert!(!bar.is_hidden());
        let ticker = ProgressTicker::spawn(bar, "Downloading files", ProgressUnit::Bytes, None);

        let logs = captured_logs(|| ticker.finish_with_message("Download complete"));
        assert!(logs.is_empty(), "{logs}");
    }

    /// A bar handed a counter has its position taken from it, rather than
    /// being advanced by the caller.
    #[tokio::test(start_paused = true)]
    async fn test_ticker_mirrors_the_counter_into_the_bar() {
        let counter = Arc::new(AtomicU64::new(0));
        let ticker = ProgressTicker::spawn(
            hidden_multi_progress().add(ProgressBar::new(100)),
            "Accumulating ref files",
            ProgressUnit::Count("ref files"),
            Some(counter.clone()),
        );

        counter.store(7, Ordering::Relaxed);
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(ticker.bar().position(), 7);
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
        copy_files_with_progress(&paths, &src_store, &dest_store, 2, &progress).await?;
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
        copy_files_with_progress(&paths, &src_store, &dest_store, 2, &progress).await?;
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

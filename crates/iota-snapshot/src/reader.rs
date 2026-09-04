// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    num::NonZeroUsize,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::{Context, Result, bail};
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use bytes::{Buf, Bytes};
use fastcrypto::hash::{HashFunction, MultisetHash, Sha3_256};
use futures::{
    StreamExt, TryStreamExt,
    future::{AbortRegistration, Abortable},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use integer_encoding::VarIntReader;
use iota_common::stream_ext::TrySpawnStreamExt;
use iota_config::object_storage_config::ObjectStoreConfig;
use iota_core::authority::authority_store_tables::{
    AuthorityPerpetualTables, LiveObject, SnapshotLiveObject,
};
use iota_sdk_types::{ObjectDigest, ObjectId, ObjectReference, Version};
use iota_storage::{
    blob::{Blob, BlobEncoding},
    object_store::{
        ObjectStoreGetExt, ObjectStoreListExt, ObjectStorePutExt,
        http::HttpDownloaderBuilder,
        util::{MANIFEST_FILENAME, RootManifest, copy_file, get_path, path_to_filesystem},
    },
};
use iota_types::{digests::ChainIdentifier, global_state_hash::GlobalStateHash};
use object_store::path::Path;
use tokio::{
    sync::{Mutex, mpsc, oneshot},
    task::JoinHandle,
};
use tracing::info;

use crate::{
    EPOCH_INFO_FILE_MAGIC, EpochInfo, FileMetadata, FileType, MAGIC_BYTES, MANIFEST_FILE_MAGIC,
    Manifest, OBJECT_FILE_MAGIC, OBJECT_ID_BYTES, OBJECT_REF_BYTES, REFERENCE_FILE_MAGIC,
    SEQUENCE_NUM_BYTES, SHA3_BYTES,
    progress::{
        DownloadProgressBar, ProgressTicker, ProgressUnit, copy_files_with_progress,
        fetch_total_bytes, get_single_file_with_progress, get_with_progress, println_or_log,
    },
    restore::Restore,
};

pub type SnapshotChecksums = (DigestByBucketAndPartition, GlobalStateHash);
pub type DigestByBucketAndPartition = BTreeMap<u32, BTreeMap<u32, [u8; 32]>>;

/// Channels through which the reader reports state accumulation: each
/// partition's partial hash goes out on `partials`, and `completion` fires
/// once every partition has been sent — so the receiving side can tell a
/// finished accumulation apart from one whose sender was dropped by a failed
/// restore.
pub struct StateAccumulatorSender {
    pub partials: mpsc::Sender<(GlobalStateHash, u64)>,
    pub completion: oneshot::Sender<()>,
}

#[derive(Clone)]
pub struct StateSnapshotReaderV1 {
    epoch: u64,
    local_staging_dir_root: PathBuf,
    remote_object_store: Arc<dyn ObjectStoreGetExt>,
    local_object_store: Arc<dyn ObjectStorePutExt>,
    local_object_store_list: Arc<dyn ObjectStoreListExt>,
    ref_files: BTreeMap<u32, BTreeMap<u32, FileMetadata>>,
    object_files: BTreeMap<u32, BTreeMap<u32, FileMetadata>>,
    epoch_info_metadata: FileMetadata,
    /// Chain identifier recorded in the snapshot's `ManifestV2`.
    chain_id: ChainIdentifier,
    multi_progress_bar: MultiProgress,
    concurrency: NonZeroUsize,
    /// When set, the staging directory is left as it is and any reference
    /// files already in it are not downloaded again.
    skip_reset_local_store: bool,
}

impl StateSnapshotReaderV1 {
    /// Reads the snapshot's MANIFEST — the metadata of its reference, object
    /// and `EPOCH_INFO` files — and prepares the local staging directory.
    ///
    /// Only the MANIFEST is downloaded here. The reference and object files
    /// both come down in [`Self::read`], so a caller can read and check
    /// [`Self::read_epoch_info`] first and reject the snapshot before any of
    /// it is transferred.
    pub async fn new(
        epoch: u64,
        remote_store_config: &ObjectStoreConfig,
        local_store_config: &ObjectStoreConfig,
        download_concurrency: NonZeroUsize,
        multi_progress_bar: MultiProgress,
        skip_reset_local_store: bool,
    ) -> Result<Self> {
        let epoch_dir = format!("epoch_{epoch}");
        let remote_object_store = make_remote_store(remote_store_config)?;
        let local_object_store: Arc<dyn ObjectStorePutExt> =
            local_store_config.make().map(Arc::new)?;
        let local_object_store_list: Arc<dyn ObjectStoreListExt> =
            local_store_config.make().map(Arc::new)?;
        let local_staging_dir_root = local_store_config
            .directory
            .as_ref()
            .context("No directory specified")?
            .clone();
        if !skip_reset_local_store {
            let local_epoch_dir_path = local_staging_dir_root.join(&epoch_dir);
            if local_epoch_dir_path.exists() {
                // A previous run's staging files can be a whole snapshot's
                // worth, so removing them is not always quick.
                println_or_log(
                    &multi_progress_bar,
                    format!(
                        "Removing the files a previous run staged in {}",
                        local_epoch_dir_path.display()
                    ),
                )?;
                fs::remove_dir_all(&local_epoch_dir_path)?;
            }
            fs::create_dir_all(&local_epoch_dir_path)?;
        }
        let epoch_dir_path = Path::from(epoch_dir);
        // Downloads MANIFEST from remote store
        println_or_log(&multi_progress_bar, "Reading the snapshot MANIFEST")?;
        let manifest_file_path = epoch_dir_path.child("MANIFEST");
        copy_file(
            &manifest_file_path,
            &manifest_file_path,
            &remote_object_store,
            &local_object_store,
        )
        .await?;
        let manifest = Self::read_manifest(path_to_filesystem(
            local_staging_dir_root.clone(),
            &manifest_file_path,
        )?)?;
        let chain_id = Self::validate_v2_manifest(&manifest, epoch)?;
        if manifest.address_length() as usize > ObjectId::LENGTH {
            bail!("Max possible address length is: {}", ObjectId::LENGTH);
        }
        // Stores the objects and references FileMetadata in MANIFEST to the local
        // directory, collecting the EPOCH_INFO entry in the same pass.
        let mut object_files = BTreeMap::new();
        let mut ref_files = BTreeMap::new();
        let mut epoch_info_files = Vec::new();
        for file_metadata in manifest.file_metadata() {
            match file_metadata.file_type {
                FileType::Object => {
                    // Gets the object FileMetadata bucket with the bucket number, or inserts a new
                    // one if it doesn't exist.
                    let entry = object_files
                        .entry(file_metadata.bucket_num)
                        .or_insert_with(BTreeMap::new);
                    // Inserts the object FileMetadata with the partition number to the bucket.
                    entry.insert(file_metadata.part_num, file_metadata.clone());
                }
                FileType::Reference => {
                    // Gets the reference FileMetadata bucket with the bucket number, or inserts a
                    // new one if it doesn't exist.
                    let entry = ref_files
                        .entry(file_metadata.bucket_num)
                        .or_insert_with(BTreeMap::new);
                    // Inserts the reference FileMetadata with the partition number to the bucket.
                    entry.insert(file_metadata.part_num, file_metadata.clone());
                }
                FileType::EpochInfo => epoch_info_files.push(file_metadata.clone()),
            }
        }
        let epoch_info_metadata = Self::single_epoch_info_metadata(epoch_info_files)?;
        Ok(StateSnapshotReaderV1 {
            epoch,
            local_staging_dir_root,
            remote_object_store,
            local_object_store,
            local_object_store_list,
            ref_files,
            object_files,
            epoch_info_metadata,
            chain_id,
            multi_progress_bar,
            concurrency: download_concurrency,
            skip_reset_local_store,
        })
    }

    /// Downloads every reference file into the local staging directory, where
    /// [`Self::ref_iter`] reads them from, and returns the bar it advanced —
    /// whose totals also cover the object files that
    /// [`Self::sync_live_objects`] downloads onto the same bar.
    async fn download_reference_files(&self) -> Result<Arc<DownloadProgressBar>> {
        let epoch_dir_path = self.epoch_dir();
        let ref_file_paths: Vec<Path> = self
            .ref_files
            .values()
            .flat_map(|entry| {
                entry
                    .values()
                    .map(|file_metadata| file_metadata.file_path(&epoch_dir_path))
            })
            .collect();

        let files_to_download = if self.skip_reset_local_store {
            let mut list_stream = self
                .local_object_store_list
                .list_objects(Some(&epoch_dir_path))
                .await;
            let mut existing_files = std::collections::HashSet::new();
            while let Some(Ok(meta)) = list_stream.next().await {
                existing_files.insert(meta.location);
            }
            let mut missing_files = Vec::new();
            for file in &ref_file_paths {
                if !existing_files.contains(file) {
                    missing_files.push(file.clone());
                }
            }
            missing_files
        } else {
            ref_file_paths
        };

        let object_file_paths: Vec<Path> = self
            .object_files
            .values()
            .flat_map(|entry| {
                entry
                    .values()
                    .map(|file_metadata| file_metadata.file_path(&epoch_dir_path))
            })
            .collect();
        let num_files = (files_to_download.len() + object_file_paths.len()) as u64;
        let total_bytes = fetch_total_bytes(
            &self.remote_object_store,
            files_to_download
                .iter()
                .cloned()
                .chain(object_file_paths)
                .collect(),
            self.concurrency.get(),
            &self.multi_progress_bar,
        )
        .await;

        let download_progress = Arc::new(DownloadProgressBar::new(
            &self.multi_progress_bar,
            "Downloading files",
            num_files,
            total_bytes,
        ));

        // Downloads all reference files from remote store to local store in parallel
        // and updates the progress bar accordingly
        copy_files_with_progress(
            &files_to_download,
            &self.remote_object_store,
            &self.local_object_store,
            self.concurrency.get(),
            &download_progress,
        )
        .await?;
        Ok(download_progress)
    }

    pub async fn read(
        &mut self,
        perpetual_db: &AuthorityPerpetualTables,
        abort_registration: AbortRegistration,
        accumulator: Option<StateAccumulatorSender>,
    ) -> Result<()> {
        self.read_to_db(perpetual_db, abort_registration, accumulator)
            .await
    }

    /// Chain identifier recorded in this snapshot's manifest.
    pub fn chain_id(&self) -> ChainIdentifier {
        self.chain_id
    }

    /// Checks the manifest is a V2 snapshot for `epoch` and returns its chain
    /// id.
    fn validate_v2_manifest(manifest: &Manifest, epoch: u64) -> anyhow::Result<ChainIdentifier> {
        let snapshot_version = manifest.snapshot_version();
        if snapshot_version != 2u8 {
            bail!(
                "Unsupported snapshot version: {snapshot_version}. Only snapshot V2 is supported."
            );
        }
        if manifest.epoch() != epoch {
            bail!("Snapshot MANIFEST is not for epoch {epoch}");
        }
        manifest.chain_id().context("V2 manifest missing chain_id")
    }

    /// Validates that a V2 manifest carried exactly one `EPOCH_INFO` entry and
    /// returns it; errors if it was absent or duplicated. Callers collect the
    /// entries in their own single pass over the manifest.
    fn single_epoch_info_metadata(mut found: Vec<FileMetadata>) -> anyhow::Result<FileMetadata> {
        match found.len() {
            0 => bail!("V2 manifest missing required EPOCH_INFO entry"),
            1 => Ok(found.pop().expect("length checked to be 1")),
            _ => bail!("Manifest contains more than one EPOCH_INFO entry"),
        }
    }

    /// Reads, verifies and decodes the snapshot's `EPOCH_INFO` file from the
    /// remote store — one file, independent of the live-object restore in
    /// [`Self::read`] and cheap enough to check before starting it.
    pub async fn read_epoch_info(&self) -> anyhow::Result<EpochInfo> {
        let epoch_info_path = self.epoch_info_metadata.file_path(&self.epoch_dir());
        let bytes = get_single_file_with_progress(
            &self.remote_object_store,
            &epoch_info_path,
            "Downloading EPOCH_INFO",
            &self.multi_progress_bar,
        )
        .await?;
        println_or_log(&self.multi_progress_bar, "Checking and decoding EPOCH_INFO")?;
        Self::decode_epoch_info(bytes, &self.epoch_info_metadata)
    }

    /// Verifies the sha3 digest against the manifest, strips the 4-byte magic
    /// header, and BCS-decodes the `EPOCH_INFO` body.
    fn decode_epoch_info(bytes: Bytes, metadata: &FileMetadata) -> anyhow::Result<EpochInfo> {
        let mut hasher = Sha3_256::default();
        hasher.update(&bytes);
        let computed = hasher.finalize().digest;
        if computed != metadata.sha3_digest {
            bail!(
                "EPOCH_INFO checksum mismatch: computed {computed:?}, expected {:?}",
                metadata.sha3_digest
            );
        }
        let mut decompressed = metadata.file_compression.bytes_decompress(bytes)?;
        let mut buf = Vec::new();
        decompressed.read_to_end(&mut buf)?;
        if buf.len() < MAGIC_BYTES {
            bail!("EPOCH_INFO file is shorter than the magic header");
        }
        let magic = BigEndian::read_u32(&buf[..MAGIC_BYTES]);
        if magic != EPOCH_INFO_FILE_MAGIC {
            bail!("EPOCH_INFO magic mismatch: got {magic:#x}, expected {EPOCH_INFO_FILE_MAGIC:#x}");
        }
        let epoch_info: EpochInfo = bcs::from_bytes(&buf[MAGIC_BYTES..])?;
        Ok(epoch_info)
    }

    /// The main entrypoint of the [StateSnapshotReaderV1].
    ///
    /// This method encapsulates the logic for several operations:
    ///
    /// 1. Downloading the snapshot's `*.ref` files into the local staging
    ///    directory.
    /// 2. Computing the partition checksums of the respective `*.ref` files.
    /// 3. Computing the partition elliptic-curve multiset hash (ECMH) in the
    ///    background, and sending the result through the given `accumulator` to
    ///    the caller. This allows to compute and verify the root hash of the
    ///    live objects encoded in the snapshot. See [`GlobalStateHash`].
    /// 4. Reading, inserting, and verifying all encoded live objects to the
    ///    given `database`.
    pub async fn read_to_db(
        &mut self,
        database: &impl Restore,
        abort_registration: AbortRegistration,
        accumulator: Option<StateAccumulatorSender>,
    ) -> Result<()> {
        // The reference files have to be local before their checksums can be
        // computed. Their download shares a bar with the object files, which
        // `sync_live_objects` downloads onto it below.
        let download_progress = self.download_reference_files().await?;

        // This computes and stores the sha3 digest of object references in REFERENCE
        // file for each bucket partition. When downloading objects, we will
        // compare sha3 digest of object references per *.obj file against this.
        // This allows us to pre-fetch object references during restoration,
        // start building the state accumulator, and fail early if the state root hash
        // doesn't match. However, we still need to ensure that objects match references
        // exactly.
        let sha3_digests: Arc<Mutex<DigestByBucketAndPartition>> =
            Arc::new(Mutex::new(BTreeMap::new()));

        // Counts the total number of partitions
        let num_part_files = self
            .ref_files
            .values()
            .map(|part_files| part_files.len())
            .sum::<usize>();

        info!("Computing checksums");
        // Creates a progress bar for checksumming
        let checksum_ticker = ProgressTicker::spawn(
            self.multi_progress_bar.add(
                ProgressBar::new(num_part_files as u64).with_style(
                    ProgressStyle::with_template(
                        "[{elapsed_precise}] {wide_bar} Checksumming ref files: {pos}/{len} \
                         ({per_sec}, ETA {eta}) — {msg}",
                    )
                    .unwrap(),
                ),
            ),
            "Checksumming ref files",
            ProgressUnit::Count("ref files"),
            None,
        );
        let checksum_progress_bar = checksum_ticker.bar().clone();

        // Iterates over all FileMetadata in the ref files by partition and build up the
        // sha3 digests mapping: (bucket, (partition, sha3_digest))
        let ref_files_iter = self.ref_files.clone().into_iter();
        futures::stream::iter(ref_files_iter)
            .flat_map(|(bucket, part_files)| {
                futures::stream::iter(
                    part_files
                        .into_iter()
                        .map(move |(part, part_file)| (bucket, part, part_file)),
                )
            })
            .try_for_each_spawned(self.concurrency.get(), |(bucket, part, _part_file)| {
                let sha3_digests = sha3_digests.clone();
                let object_files = self.object_files.clone();
                let bar = checksum_progress_bar.clone();
                let this = self.clone();

                async move {
                    let ref_iter = this.ref_iter(bucket, part)?;
                    let mut hasher = Sha3_256::default();
                    let mut empty = true;

                    object_files
                        .get(&bucket)
                        .context(format!("No bucket exists for: {bucket}"))?
                        .get(&part)
                        .context(format!("No part exists for bucket: {bucket}, part: {part}"))?;

                    for object_ref in ref_iter {
                        hasher.update(object_ref.digest.bytes());
                        empty = false;
                    }

                    if !empty {
                        let mut digests = sha3_digests.lock().await;
                        digests
                            .entry(bucket)
                            .or_insert(BTreeMap::new())
                            .entry(part)
                            .or_insert(hasher.finalize().digest);
                    }

                    bar.inc(1);
                    bar.set_message(format!("Bucket: {bucket}, Part: {part}"));
                    Ok::<(), anyhow::Error>(())
                }
            })
            .await?;

        checksum_ticker.finish_with_message("Checksumming complete");

        let accum_handle = accumulator
            .map(|accumulator| self.spawn_accumulation_tasks(accumulator, num_part_files));

        // Downloads all object files from remote in parallel and inserts the objects
        // into the database of choice
        self.sync_live_objects(
            database,
            abort_registration,
            sha3_digests,
            download_progress,
        )
        .await?;

        if let Some(handle) = accum_handle {
            handle.await?;
        }
        Ok(())
    }

    /// Spawns accumulation tasks to accumulate the sha3 digests of all objects
    /// then sends the accumulator to the sender.
    fn spawn_accumulation_tasks(
        &self,
        accumulator: StateAccumulatorSender,
        num_part_files: usize,
    ) -> JoinHandle<()> {
        let StateAccumulatorSender {
            partials: sender,
            completion,
        } = accumulator;
        // Spawns accumulation progress bar, whose position the ticker takes
        // from the counter the accumulation task below advances
        let concurrency = self.concurrency;
        let accum_counter = Arc::new(AtomicU64::new(0));
        let accum_ticker = ProgressTicker::spawn(
            self.multi_progress_bar.add(
                ProgressBar::new(num_part_files as u64).with_style(
                    ProgressStyle::with_template(
                        "[{elapsed_precise}] {wide_bar} Accumulating ref files: {pos}/{len} \
                         ({per_sec}, ETA {eta})",
                    )
                    .unwrap(),
                ),
            ),
            "Accumulating ref files",
            ProgressUnit::Count("ref files"),
            Some(accum_counter.clone()),
        );

        // spawns accumualation task
        let ref_files = self.ref_files.clone();
        let epoch_dir = self.epoch_dir();
        let local_staging_dir_root = self.local_staging_dir_root.clone();
        tokio::task::spawn(async move {
            let local_staging_dir_root_clone = local_staging_dir_root.clone();
            let epoch_dir_clone = epoch_dir.clone();
            for (bucket, part_files) in ref_files.clone().iter() {
                futures::stream::iter(part_files.iter())
                    .map(|(part, _part_files)| {
                        // TODO depending on concurrency limit here, we may be
                        // materializing too many refs into memory at once.

                        // Takes the sha3 digests of every object in the partition
                        // This is only done because ObjectRefIter is not Send
                        let obj_digests = {
                            // TODO: Make sure that we can remove this getter, just take _part_files
                            // here
                            let file_metadata = ref_files
                                .get(bucket)
                                .expect("No ref files found for bucket: {bucket_num}")
                                .get(part)
                                .expect(
                                    "No ref files found for bucket: {bucket_num}, part: {part_num}",
                                );
                            ObjectRefIter::new(
                                file_metadata,
                                local_staging_dir_root_clone.clone(),
                                epoch_dir_clone.clone(),
                            )
                            .expect("Failed to create object ref iter")
                        }
                        .map(|obj_ref| obj_ref.digest)
                        .collect::<Vec<ObjectDigest>>();

                        // Spawns a task to accumulate the sha3 digests and send the accumulator
                        // to the sender.
                        let sender_clone = sender.clone();
                        tokio::spawn(async move {
                            let mut partial_acc = GlobalStateHash::default();
                            let num_objects = obj_digests.len();
                            partial_acc.insert_all(obj_digests);
                            sender_clone
                                .send((partial_acc, num_objects as u64))
                                .await
                                .expect("Unable to send accumulator from snapshot reader");
                        })
                    })
                    .boxed()
                    .buffer_unordered(concurrency.into())
                    .for_each(|result| {
                        // Update the progress bar
                        result.expect("Failed to generate partial accumulator");
                        accum_counter.fetch_add(1, Ordering::Relaxed);
                        futures::future::ready(())
                    })
                    .await;
            }
            accum_ticker.finish_with_message("Accumulation complete");
            let _ = completion.send(());
        })
    }

    /// Downloads all object files from remote in parallel and inserts the
    /// objects into the given database.
    async fn sync_live_objects(
        &self,
        database: &impl Restore,
        abort_registration: AbortRegistration,
        sha3_digests: Arc<Mutex<DigestByBucketAndPartition>>,
        download_progress: Arc<DownloadProgressBar>,
    ) -> Result<(), anyhow::Error> {
        let epoch_dir = self.epoch_dir();
        let concurrency = self.concurrency;
        let remote_object_store = self.remote_object_store.clone();
        // collects a vector of all object FileMetadata in the form of:
        // (bucket, (partition, File_metadata))
        let input_files: Vec<_> = self
            .object_files
            .iter()
            .flat_map(|(bucket, parts)| {
                parts
                    .clone()
                    .into_iter()
                    .map(|entry| (bucket, entry))
                    .collect::<Vec<_>>()
            })
            .collect();
        // Continues the bar the reference files were downloaded on, whose
        // totals already include these object files.
        let obj_progress = download_progress;
        let obj_progress_clone = obj_progress.clone();
        let obj_progress_download = obj_progress.clone();

        let ret = Abortable::new(
            async move {
                // Downloads all object files from remote store to local store in parallel
                // and inserts the objects into the AuthorityPerpetualTables
                futures::stream::iter(input_files.iter())
                    .map(|(bucket, (part_num, file_metadata))| {
                        let epoch_dir = epoch_dir.clone();
                        let file_path = file_metadata.file_path(&epoch_dir);
                        let remote_object_store = remote_object_store.clone();
                        let sha3_digests_cloned = sha3_digests.clone();
                        let obj_progress = obj_progress_download.clone();
                        async move {
                            // Downloads object file with retries
                            let bytes =
                                get_with_progress(&remote_object_store, &file_path, &obj_progress)
                                    .await
                                    .with_context(|| {
                                        format!("Failed to download object file {file_path}")
                                    })?;

                            // Gets the sha3 digest of the partition
                            let sha3_digest = sha3_digests_cloned.lock().await;
                            let bucket_map = sha3_digest
                                .get(bucket)
                                .expect("Bucket not in digest map")
                                .clone();
                            let sha3_digest = *bucket_map
                                .get(part_num)
                                .expect("sha3 digest not in bucket map");
                            Ok::<(Bytes, FileMetadata, [u8; 32]), anyhow::Error>((
                                bytes,
                                (*file_metadata).clone(),
                                sha3_digest,
                            ))
                        }
                    })
                    .boxed()
                    .buffer_unordered(concurrency.into())
                    .try_for_each(|(bytes, file_metadata, sha3_digest)| {
                        let obj_progress = &obj_progress_clone;
                        async move {
                            database
                                .insert_partition(file_metadata, bytes, &sha3_digest)
                                .await?;
                            obj_progress.file_done();
                            Ok(())
                        }
                    })
                    .await
            },
            abort_registration,
        )
        .await?;
        obj_progress.finish_with_message("Download complete");
        ret
    }

    /// Returns an iterator over all references in a .ref file.
    pub fn ref_iter(&self, bucket_num: u32, part_num: u32) -> Result<ObjectRefIter> {
        // Gets the reference file metadata for the {bucket_num}_{part_num}
        let file_metadata = self
            .ref_files
            .get(&bucket_num)
            .context(format!("No ref files found for bucket: {bucket_num}"))?
            .get(&part_num)
            .context(format!(
                "No ref files found for bucket: {bucket_num}, part: {part_num}"
            ))?;
        ObjectRefIter::new(
            file_metadata,
            self.local_staging_dir_root.clone(),
            self.epoch_dir(),
        )
    }

    /// Returns a list of all buckets.
    fn buckets(&self) -> Result<Vec<u32>> {
        Ok(self.ref_files.keys().copied().collect())
    }

    fn epoch_dir(&self) -> Path {
        Path::from(format!("epoch_{}", self.epoch))
    }

    /// Reads the MANIFEST file, verifies it with the checksum, and returns the
    /// Manifest.
    fn read_manifest(path: PathBuf) -> anyhow::Result<Manifest> {
        let mut buf = Vec::new();
        File::open(path)?.read_to_end(&mut buf)?;
        Self::read_manifest_from_bytes(&buf)
    }

    /// Parses a snapshot MANIFEST from its raw on-disk bytes: validates the
    /// magic header, verifies the trailing sha3 digest, and BCS-decodes it.
    fn read_manifest_from_bytes(buf: &[u8]) -> anyhow::Result<Manifest> {
        if buf.len() < MAGIC_BYTES + SHA3_BYTES {
            bail!("MANIFEST is shorter than the magic header plus checksum");
        }
        let magic = BigEndian::read_u32(&buf[..MAGIC_BYTES]);
        if magic != MANIFEST_FILE_MAGIC {
            bail!("Unexpected magic byte: {magic}");
        }
        // The sha3 digest of the body is the trailing `SHA3_BYTES`.
        let (content, sha3_digest) = buf.split_at(buf.len() - SHA3_BYTES);
        let mut hasher = Sha3_256::default();
        hasher.update(content);
        let computed_digest = hasher.finalize().digest;
        if computed_digest != sha3_digest {
            bail!("Checksum: {computed_digest:?} don't match: {sha3_digest:?}");
        }
        let manifest = bcs::from_bytes(&content[MAGIC_BYTES..])?;
        Ok(manifest)
    }
}

/// Builds the remote object store handle, using the unsigned HTTP path when
/// `no_sign_request` is set.
fn make_remote_store(config: &ObjectStoreConfig) -> anyhow::Result<Arc<dyn ObjectStoreGetExt>> {
    if config.no_sign_request {
        config.make_http()
    } else {
        Ok(Arc::new(config.make()?))
    }
}

/// The latest formal-snapshot epoch published to `remote_store_config`, read
/// from the bucket's root MANIFEST. The MANIFEST lists only completed
/// snapshots, so the returned epoch is always safe to read.
pub async fn latest_available_epoch(
    remote_store_config: &ObjectStoreConfig,
) -> anyhow::Result<u64> {
    let remote_object_store = make_remote_store(remote_store_config)?;
    let bytes = remote_object_store
        .get_bytes(&get_path(MANIFEST_FILENAME))
        .await?;
    let manifest = RootManifest::from_bytes(&bytes)?;
    manifest
        .available_epochs
        .iter()
        .map(|(epoch, _)| *epoch)
        .max()
        .ok_or_else(|| anyhow::anyhow!("no snapshot found in root MANIFEST"))
}

/// An iterator over all object refs in a .ref file.
pub struct ObjectRefIter {
    reader: Box<dyn Read>,
}

impl ObjectRefIter {
    pub fn new(file_metadata: &FileMetadata, root_path: PathBuf, dir_path: Path) -> Result<Self> {
        let file_path = file_metadata.local_file_path(&root_path, &dir_path)?;
        let mut reader = file_metadata.file_compression.decompress(&file_path)?;
        let magic = reader.read_u32::<BigEndian>()?;
        if magic != REFERENCE_FILE_MAGIC {
            bail!("Unexpected magic string in REFERENCE file: {magic:?}")
        } else {
            Ok(ObjectRefIter { reader })
        }
    }

    fn next_ref(&mut self) -> Result<ObjectReference> {
        let mut buf = [0u8; OBJECT_REF_BYTES];
        self.reader.read_exact(&mut buf)?;
        let object_id = &buf[0..OBJECT_ID_BYTES];
        let version = &buf[OBJECT_ID_BYTES..OBJECT_ID_BYTES + SEQUENCE_NUM_BYTES]
            .reader()
            .read_u64::<BigEndian>()?;
        let sha3_digest = &buf[OBJECT_ID_BYTES + SEQUENCE_NUM_BYTES..OBJECT_REF_BYTES];
        let object_ref = ObjectReference::new(
            ObjectId::from_bytes(object_id)?,
            Version::from_u64(*version),
            ObjectDigest::from_bytes(sha3_digest)?,
        );
        Ok(object_ref)
    }
}

impl Iterator for ObjectRefIter {
    type Item = ObjectReference;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_ref().ok()
    }
}

/// An iterator over all objects in a *.obj file.
pub struct LiveObjectIter {
    reader: Box<dyn Read>,
}

impl LiveObjectIter {
    pub fn new(file_metadata: &FileMetadata, bytes: Bytes) -> Result<Self> {
        let mut reader = file_metadata.file_compression.bytes_decompress(bytes)?;
        let magic = reader.read_u32::<BigEndian>()?;
        if magic != OBJECT_FILE_MAGIC {
            bail!("Unexpected magic string in object file: {magic:?}")
        } else {
            Ok(LiveObjectIter { reader })
        }
    }

    fn next_object(&mut self) -> Result<LiveObject> {
        let len = self.reader.read_varint::<u64>()? as usize;
        if len == 0 {
            bail!("Invalid object length of 0 in file");
        }
        let encoding = self.reader.read_u8()?;
        let mut data = vec![0u8; len];
        self.reader.read_exact(&mut data)?;
        let blob = Blob {
            data,
            encoding: BlobEncoding::try_from(encoding)?,
        };
        let snap: SnapshotLiveObject = blob.decode()?;
        Ok(snap.into())
    }
}

impl Iterator for LiveObjectIter {
    type Item = LiveObject;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_object().ok()
    }
}

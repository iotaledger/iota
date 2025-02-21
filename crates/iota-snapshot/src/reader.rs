// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    fs,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    num::NonZeroUsize,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

use anyhow::{Context, Result, anyhow};
use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, Bytes};
use fastcrypto::hash::{HashFunction, MultisetHash, Sha3_256};
use futures::{
    StreamExt, TryStreamExt,
    future::{AbortRegistration, Abortable},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use integer_encoding::VarIntReader;
use iota_config::object_storage_config::ObjectStoreConfig;
use iota_core::authority::{
    AuthorityStore,
    authority_store_tables::{AuthorityPerpetualTables, LiveObject},
};
use iota_storage::{
    blob::{Blob, BlobEncoding},
    object_store::{
        ObjectStoreGetExt, ObjectStorePutExt,
        http::HttpDownloaderBuilder,
        util::{copy_file, copy_files, path_to_filesystem},
    },
};
use iota_types::{
    accumulator::Accumulator,
    base_types::{ObjectDigest, ObjectID, ObjectRef, SequenceNumber},
};
use object_store::path::Path;
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{Duration, Instant},
};
use tracing::{error, info};

use crate::{
    FileMetadata, FileType, MAGIC_BYTES, MANIFEST_FILE_MAGIC, Manifest, OBJECT_FILE_MAGIC,
    OBJECT_ID_BYTES, OBJECT_REF_BYTES, REFERENCE_FILE_MAGIC, SEQUENCE_NUM_BYTES, SHA3_BYTES,
};

pub type SnapshotChecksums = (DigestByBucketAndPartition, Accumulator);
pub type DigestByBucketAndPartition = BTreeMap<u32, BTreeMap<u32, [u8; 32]>>;
pub struct StateSnapshotReaderV1 {
    epoch: u64,
    local_staging_dir_root: PathBuf,
    remote_object_store: Arc<dyn ObjectStoreGetExt>,
    local_object_store: Arc<dyn ObjectStorePutExt>,
    ref_files: BTreeMap<u32, BTreeMap<u32, FileMetadata>>,
    object_files: BTreeMap<u32, BTreeMap<u32, FileMetadata>>,
    indirect_objects_threshold: usize,
    multi_progress_bar: MultiProgress,
    concurrency: usize,
}

impl StateSnapshotReaderV1 {
    /// Downloads the MANIFEST, FileMetadata of objects and references from the
    /// remote store, then creates a StateSnapshotReaderV1 instance.
    pub async fn new(
        epoch: u64,
        remote_store_config: &ObjectStoreConfig,
        local_store_config: &ObjectStoreConfig,
        indirect_objects_threshold: usize,
        download_concurrency: NonZeroUsize,
        multi_progress_bar: MultiProgress,
    ) -> Result<Self> {
        let epoch_dir = format!("epoch_{}", epoch);
        let remote_object_store = if remote_store_config.no_sign_request {
            remote_store_config.make_http()?
        } else {
            remote_store_config.make().map(Arc::new)?
        };
        let local_object_store: Arc<dyn ObjectStorePutExt> =
            local_store_config.make().map(Arc::new)?;
        let local_staging_dir_root = local_store_config
            .directory
            .as_ref()
            .context("No directory specified")?
            .clone();
        let local_epoch_dir_path = local_staging_dir_root.join(&epoch_dir);
        if local_epoch_dir_path.exists() {
            fs::remove_dir_all(&local_epoch_dir_path)?;
        }
        fs::create_dir_all(&local_epoch_dir_path)?;
        // Downloads MANIFEST from remote store
        let manifest_file_path = Path::from(epoch_dir.clone()).child("MANIFEST");
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
        // Verifies MANIFEST
        let snapshot_version = manifest.snapshot_version();
        if snapshot_version != 1u8 {
            return Err(anyhow!("Unexpected snapshot version: {}", snapshot_version));
        }
        if manifest.address_length() as usize > ObjectID::LENGTH {
            return Err(anyhow!(
                "Max possible address length is: {}",
                ObjectID::LENGTH
            ));
        }
        if manifest.epoch() != epoch {
            return Err(anyhow!("Download manifest is not for epoch: {}", epoch,));
        }
        // Stores the objects and references FileMetadata in MANIFEST to the local
        // directory
        let mut object_files = BTreeMap::new();
        let mut ref_files = BTreeMap::new();
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
            }
        }
        let epoch_dir_path = Path::from(epoch_dir);
        // Collects the path of all reference files
        let files: Vec<Path> = ref_files
            .values()
            .flat_map(|entry| {
                let files: Vec<_> = entry
                    .values()
                    .map(|file_metadata| file_metadata.file_path(&epoch_dir_path))
                    .collect();
                files
            })
            .collect();

        let progress_bar = multi_progress_bar.add(
            ProgressBar::new(files.len() as u64).with_style(
                ProgressStyle::with_template(
                    "[{elapsed_precise}] {wide_bar} {pos} out of {len} .ref files done ({msg})",
                )
                .unwrap(),
            ),
        );
        // Downloads all reference files from remote store to local store in parallel
        // and updates the progress bar accordingly
        copy_files(
            &files,
            &files,
            &remote_object_store,
            &local_object_store,
            download_concurrency,
            Some(progress_bar.clone()),
        )
        .await?;
        progress_bar.finish_with_message("ref files download complete");
        Ok(StateSnapshotReaderV1 {
            epoch,
            local_staging_dir_root,
            remote_object_store,
            local_object_store,
            ref_files,
            object_files,
            indirect_objects_threshold,
            multi_progress_bar,
            concurrency: download_concurrency.get(),
        })
    }

    pub async fn read(
        &mut self,
        perpetual_db: &AuthorityPerpetualTables,
        abort_registration: AbortRegistration,
        sender: Option<tokio::sync::mpsc::Sender<(Accumulator, u64)>>,
    ) -> Result<()> {
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
        let checksum_progress_bar = self.multi_progress_bar.add(
            ProgressBar::new(num_part_files as u64).with_style(
                ProgressStyle::with_template(
                    "[{elapsed_precise}] {wide_bar} {pos} out of {len} ref files checksummed ({msg})",
                )
                .unwrap(),
            ),
        );

        // Iterates over all FileMetadata in the ref files by partition and build up the
        // sha3 digests mapping: (bucket, (partition, sha3_digest))
        for (bucket, part_files) in self.ref_files.clone().iter() {
            for (part, _part_file) in part_files.iter() {
                let mut sha3_digests = sha3_digests.lock().await;
                let ref_iter = self.ref_iter(*bucket, *part)?;
                let mut hasher = Sha3_256::default();
                let mut empty = true;
                // TODO: This can be removed, the same operation is done in ref_iter() in line
                // 238
                self.object_files
                    .get(bucket)
                    .context(format!("No bucket exists for: {bucket}"))?
                    .get(part)
                    .context(format!("No part exists for bucket: {bucket}, part: {part}"))?;
                // Inserts the sha3 digest of each object into the hasher
                for object_ref in ref_iter {
                    hasher.update(object_ref.2.inner());
                    empty = false;
                }
                // Computes the sha3 digest of the partition and insert sit into the
                // sha3_digests map
                if !empty {
                    sha3_digests
                        .entry(*bucket)
                        .or_insert(BTreeMap::new())
                        .entry(*part)
                        .or_insert(hasher.finalize().digest);
                }
                checksum_progress_bar.inc(1);
                checksum_progress_bar.set_message(format!("Bucket: {}, Part: {}", bucket, part));
            }
        }
        checksum_progress_bar.finish_with_message("Checksumming complete");

        let accum_handle =
            sender.map(|sender| self.spawn_accumulation_tasks(sender, num_part_files));

        // Downloads all object files from remote in parallel and inserts the objects
        // into the AuthorityPerpetualTables
        self.sync_live_objects(perpetual_db, abort_registration, sha3_digests)
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
        sender: tokio::sync::mpsc::Sender<(Accumulator, u64)>,
        num_part_files: usize,
    ) -> JoinHandle<()> {
        // Spawns accumulation progress bar
        let concurrency = self.concurrency;
        let accum_counter = Arc::new(AtomicU64::new(0));
        let cloned_accum_counter = accum_counter.clone();
        let accum_progress_bar = self.multi_progress_bar.add(
             ProgressBar::new(num_part_files as u64).with_style(
                 ProgressStyle::with_template(
                     "[{elapsed_precise}] {wide_bar} {pos} out of {len} ref files accumulated from snapshot ({msg})",
                 )
                 .unwrap(),
             ),
         );
        let cloned_accum_progress_bar = accum_progress_bar.clone();
        // Spawns accumulation progress bar update task
        tokio::spawn(async move {
            let a_instant = Instant::now();
            loop {
                if cloned_accum_progress_bar.is_finished() {
                    break;
                }
                let num_partitions = cloned_accum_counter.load(Ordering::Relaxed);
                let total_partitions_per_sec =
                    num_partitions as f64 / a_instant.elapsed().as_secs_f64();
                cloned_accum_progress_bar.set_position(num_partitions);
                cloned_accum_progress_bar.set_message(format!(
                    "file partitions per sec: {}",
                    total_partitions_per_sec
                ));
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

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
                        .map(|obj_ref| obj_ref.2)
                        .collect::<Vec<ObjectDigest>>();

                        // Spawns a task to accumulate the sha3 digests and send the accumulator
                        // to the sender.
                        let sender_clone = sender.clone();
                        tokio::spawn(async move {
                            let mut partial_acc = Accumulator::default();
                            let num_objects = obj_digests.len();
                            partial_acc.insert_all(obj_digests);
                            sender_clone
                                .send((partial_acc, num_objects as u64))
                                .await
                                .expect("Unable to send accumulator from snapshot reader");
                        })
                    })
                    .boxed()
                    .buffer_unordered(concurrency)
                    .for_each(|result| {
                        // Update the progress bar
                        result.expect("Failed to generate partial accumulator");
                        accum_counter.fetch_add(1, Ordering::Relaxed);
                        futures::future::ready(())
                    })
                    .await;
            }
            accum_progress_bar.finish_with_message("Accumulation complete");
        })
    }

    /// Downloads all object files from remote in parallel and inserts the
    /// objects into the AuthorityPerpetualTables.
    async fn sync_live_objects(
        &self,
        perpetual_db: &AuthorityPerpetualTables,
        abort_registration: AbortRegistration,
        sha3_digests: Arc<Mutex<DigestByBucketAndPartition>>,
    ) -> Result<(), anyhow::Error> {
        let epoch_dir = self.epoch_dir();
        let concurrency = self.concurrency;
        let threshold = self.indirect_objects_threshold;
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
        // Creates a progress bar for object files
        let obj_progress_bar = self.multi_progress_bar.add(
            ProgressBar::new(input_files.len() as u64).with_style(
                ProgressStyle::with_template(
                    "[{elapsed_precise}] {wide_bar} {pos} out of {len} .obj files done ({msg})",
                )
                .unwrap(),
            ),
        );
        let obj_progress_bar_clone = obj_progress_bar.clone();
        let instant = Instant::now();
        let downloaded_bytes = AtomicUsize::new(0);

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
                        async move {
                            // Downloads object file with retries
                            let max_timeout = Duration::from_secs(60);
                            let mut timeout = Duration::from_secs(2);
                            timeout += timeout / 2;
                            timeout = std::cmp::min(max_timeout, timeout);
                            let mut attempts = 0usize;
                            let bytes = loop {
                                match remote_object_store.get_bytes(&file_path).await {
                                    Ok(bytes) => {
                                        break bytes;
                                    }
                                    Err(err) => {
                                        error!(
                                            "Obj {} .get failed (attempt {}): {}",
                                            file_metadata.file_path(&epoch_dir),
                                            attempts,
                                            err,
                                        );
                                        if timeout > max_timeout {
                                            panic!(
                                                "Failed to get obj file {} after {} attempts",
                                                file_metadata.file_path(&epoch_dir),
                                                attempts,
                                            );
                                        } else {
                                            attempts += 1;
                                            tokio::time::sleep(timeout).await;
                                            timeout += timeout / 2;
                                            continue;
                                        }
                                    }
                                }
                            };

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
                    .buffer_unordered(concurrency)
                    .try_for_each(|(bytes, file_metadata, sha3_digest)| {
                        let bytes_len = bytes.len();
                        // Inserts live objects into the AuthorityStore
                        let result: Result<(), anyhow::Error> =
                            LiveObjectIter::new(&file_metadata, bytes).map(|obj_iter| {
                                AuthorityStore::bulk_insert_live_objects(
                                    perpetual_db,
                                    obj_iter,
                                    threshold,
                                    &sha3_digest,
                                )
                                .expect("Failed to insert live objects");
                            });
                        downloaded_bytes.fetch_add(bytes_len, Ordering::Relaxed);
                        // Updates the progress bar
                        obj_progress_bar_clone.inc(1);
                        obj_progress_bar_clone.set_message(format!(
                            "Download speed: {} MiB/s",
                            downloaded_bytes.load(Ordering::Relaxed) as f64
                                / (1024 * 1024) as f64
                                / instant.elapsed().as_secs_f64(),
                        ));
                        futures::future::ready(result)
                    })
                    .await
            },
            abort_registration,
        )
        .await?;
        obj_progress_bar.finish_with_message("Objects download complete");
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
        let manifest_file = File::open(path)?;
        let manifest_file_size = manifest_file.metadata()?.len() as usize;
        let mut manifest_reader = BufReader::new(manifest_file);
        // Make sure the file is MANIFEST with correct magic bytes
        manifest_reader.rewind()?;
        let magic = manifest_reader.read_u32::<BigEndian>()?;
        if magic != MANIFEST_FILE_MAGIC {
            return Err(anyhow!("Unexpected magic byte: {}", magic));
        }
        // Gets the sha3 digest from the end of the file
        manifest_reader.seek(SeekFrom::End(-(SHA3_BYTES as i64)))?;
        let mut sha3_digest = [0u8; SHA3_BYTES];
        manifest_reader.read_exact(&mut sha3_digest)?;
        // Rewinds to the beginning of the file and read the contents
        manifest_reader.rewind()?;
        let mut content_buf = vec![0u8; manifest_file_size - SHA3_BYTES];
        manifest_reader.read_exact(&mut content_buf)?;
        // Computes the sha3 digest of the content and check if it matches the one at
        // the end
        let mut hasher = Sha3_256::default();
        hasher.update(&content_buf);
        let computed_digest = hasher.finalize().digest;
        if computed_digest != sha3_digest {
            return Err(anyhow!(
                "Checksum: {:?} don't match: {:?}",
                computed_digest,
                sha3_digest
            ));
        }
        manifest_reader.rewind()?;
        manifest_reader.seek(SeekFrom::Start(MAGIC_BYTES as u64))?;
        let manifest = bcs::from_bytes(&content_buf[MAGIC_BYTES..])?;
        Ok(manifest)
    }
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
            Err(anyhow!(
                "Unexpected magic string in REFERENCE file: {:?}",
                magic
            ))
        } else {
            Ok(ObjectRefIter { reader })
        }
    }

    fn next_ref(&mut self) -> Result<ObjectRef> {
        let mut buf = [0u8; OBJECT_REF_BYTES];
        self.reader.read_exact(&mut buf)?;
        let object_id = &buf[0..OBJECT_ID_BYTES];
        let sequence_number = &buf[OBJECT_ID_BYTES..OBJECT_ID_BYTES + SEQUENCE_NUM_BYTES]
            .reader()
            .read_u64::<BigEndian>()?;
        let sha3_digest = &buf[OBJECT_ID_BYTES + SEQUENCE_NUM_BYTES..OBJECT_REF_BYTES];
        let object_ref: ObjectRef = (
            ObjectID::from_bytes(object_id)?,
            SequenceNumber::from_u64(*sequence_number),
            ObjectDigest::try_from(sha3_digest)?,
        );
        Ok(object_ref)
    }
}

impl Iterator for ObjectRefIter {
    type Item = ObjectRef;
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
            Err(anyhow!(
                "Unexpected magic string in object file: {:?}",
                magic
            ))
        } else {
            Ok(LiveObjectIter { reader })
        }
    }

    fn next_object(&mut self) -> Result<LiveObject> {
        let len = self.reader.read_varint::<u64>()? as usize;
        if len == 0 {
            return Err(anyhow!("Invalid object length of 0 in file"));
        }
        let encoding = self.reader.read_u8()?;
        let mut data = vec![0u8; len];
        self.reader.read_exact(&mut data)?;
        let blob = Blob {
            data,
            encoding: BlobEncoding::try_from(encoding)?,
        };
        blob.decode()
    }
}

impl Iterator for LiveObjectIter {
    type Item = LiveObject;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_object().ok()
    }
}

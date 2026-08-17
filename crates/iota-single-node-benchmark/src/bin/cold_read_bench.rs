// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Standalone cold-read microbenchmark for gas-metering calibration.
//!
//! Measures the store-level cost of object reads — fetch from RocksDB plus
//! deserialization — independently of transaction execution. The integrated
//! benchmark sweeps measure the warm in-execution read cost; the cold
//! coefficient composes as that warm cost plus the cold-minus-warm fetch
//! delta measured here.
//!
//! Two phases, two processes:
//!
//! ```sh
//! cold-read-bench populate --db-path DIR --num-objects N --object-size S
//! # (optionally drop the OS page cache here)
//! cold-read-bench measure  --db-path DIR --num-objects N --sample K --out F
//! ```
//!
//! The measure process regenerates object IDs from the shared seed instead
//! of scanning the store — a scan would warm exactly the caches this tool
//! exists to keep cold. A fresh process gives a cold RocksDB block cache;
//! dropping the OS page cache between phases (runner's job) makes the disk
//! cold too. Each measure pass reads the same IDs twice: the first pass is
//! the cold sample, the second the in-process warm contrast.

use std::{fs::File, io::Write, path::PathBuf, time::Instant};

use clap::{Parser, Subcommand};
use iota_core::authority::authority_store_tables::AuthorityPerpetualTables;
use iota_sdk_types::{Address, ObjectId, Owner, StructTag, TransactionDigest};
use iota_types::object::{MoveStructExt, Object};
use rand::{Rng, SeedableRng, rngs::StdRng};

#[derive(Parser)]
#[command(
    name = "cold-read-bench",
    about = "Store-level cold read microbenchmark"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Fill a fresh store with deterministic objects.
    Populate {
        #[arg(long)]
        db_path: PathBuf,
        #[arg(long)]
        num_objects: u64,
        #[arg(long, default_value_t = 1024)]
        object_size: u64,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        /// Also store the built-in framework packages (0x1, 0x2) as package
        /// objects, for the package fetch+deserialize measurement.
        #[arg(long, default_value_t = false)]
        with_framework_packages: bool,
    },
    /// Read a sample of the populated objects, cold then warm, and write one
    /// JSON line per read.
    Measure {
        #[arg(long)]
        db_path: PathBuf,
        #[arg(long)]
        num_objects: u64,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value_t = 1000)]
        sample: u64,
        #[arg(long)]
        out: PathBuf,
        /// Also fetch + deserialize the framework packages stored by
        /// `populate --with-framework-packages`.
        #[arg(long, default_value_t = false)]
        packages: bool,
    },
}

/// The object IDs are a pure function of (seed, num_objects), shared by both
/// phases.
fn object_ids(seed: u64, num_objects: u64) -> Vec<ObjectId> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..num_objects)
        .map(|_| ObjectId::new(rng.r#gen()))
        .collect()
}

fn blob_type() -> StructTag {
    "0xb0b::bench::Blob".parse().unwrap()
}

fn populate(
    db_path: PathBuf,
    num_objects: u64,
    object_size: u64,
    seed: u64,
    with_framework_packages: bool,
) {
    assert!(object_size >= 32, "object size must fit the 32-byte id");
    assert!(
        !db_path.exists() || db_path.read_dir().unwrap().next().is_none(),
        "refusing to populate a non-empty --db-path"
    );
    let tables = AuthorityPerpetualTables::open(&db_path, None);
    let owner = Owner::Address(Address::new([7; 32]));
    let ids = object_ids(seed, num_objects);
    for (i, id) in ids.iter().enumerate() {
        let mut contents = id.as_bytes().to_vec();
        contents.resize(object_size as usize, 7u8);
        let move_object = iota_sdk_types::MoveStruct::new_from_execution_with_limit(
            blob_type(),
            1.into(),
            contents,
            u64::MAX,
        )
        .unwrap();
        let object = Object::new_move(move_object, owner.clone(), TransactionDigest::ZERO);
        tables
            .insert_store_object_v2_test_only(object, None)
            .unwrap();
        if (i + 1) % 100_000 == 0 {
            println!("inserted {} / {num_objects}", i + 1);
        }
    }
    if with_framework_packages {
        for object in iota_framework::BuiltInFramework::genesis_objects() {
            let id = object.id();
            tables
                .insert_store_object_v2_test_only(object, None)
                .unwrap();
            println!("stored framework package {id}");
        }
    }
    println!(
        "populated {num_objects} objects of {object_size} bytes (seed {seed}) at {}",
        db_path.display()
    );
}

struct ReadTiming {
    fetch_ns: u64,
    construct_ns: u64,
    bytes: u64,
}

fn timed_read(tables: &AuthorityPerpetualTables, id: ObjectId) -> ReadTiming {
    let start = Instant::now();
    let entry = tables
        .get_latest_object_or_tombstone(id)
        .unwrap()
        .unwrap_or_else(|| panic!("populated object {id} not found; seed/num-objects mismatch?"));
    let fetch_ns = start.elapsed().as_nanos() as u64;
    let (key, wrapper) = entry;
    let start = Instant::now();
    let object = tables
        .object(&key, wrapper)
        .unwrap()
        .expect("wrapper must deserialize");
    let construct_ns = start.elapsed().as_nanos() as u64;
    ReadTiming {
        fetch_ns,
        construct_ns,
        bytes: object.object_size_for_gas_metering() as u64,
    }
}

fn median(mut xs: Vec<u64>) -> u64 {
    xs.sort_unstable();
    xs[xs.len() / 2]
}

fn measure(
    db_path: PathBuf,
    num_objects: u64,
    seed: u64,
    sample: u64,
    out_path: PathBuf,
    packages: bool,
) {
    let tables = AuthorityPerpetualTables::open(&db_path, None);
    let mut out = File::create(&out_path).unwrap();

    // Sample IDs in an order unrelated to insertion order.
    let mut ids = object_ids(seed, num_objects);
    let mut shuffle_rng = StdRng::seed_from_u64(seed.wrapping_add(1));
    for i in (1..ids.len()).rev() {
        ids.swap(i, shuffle_rng.gen_range(0..=i));
    }
    ids.truncate(sample as usize);

    writeln!(
        out,
        "{}",
        serde_json::json!({"meta": {
            "db_path": db_path.display().to_string(),
            "num_objects": num_objects,
            "seed": seed,
            "sample": sample,
        }})
    )
    .unwrap();

    for pass in ["cold", "warm"] {
        let mut fetch = Vec::with_capacity(ids.len());
        for id in &ids {
            let t = timed_read(&tables, *id);
            writeln!(
                out,
                "{}",
                serde_json::json!({
                    "kind": "object", "pass": pass, "object_id": id.to_string(),
                    "fetch_ns": t.fetch_ns, "construct_ns": t.construct_ns, "bytes": t.bytes,
                })
            )
            .unwrap();
            fetch.push(t.fetch_ns);
        }
        println!(
            "{pass}: median fetch {} ns over {} reads",
            median(fetch),
            ids.len()
        );
    }

    if packages {
        for pass in ["cold", "warm"] {
            for id in iota_framework::BuiltInFramework::all_package_ids() {
                let t = timed_read(&tables, id);
                // Deserialize every module, the CPU share of a cold package
                // load (bytecode verification is not yet included).
                let entry = tables.get_latest_object_or_tombstone(id).unwrap().unwrap();
                let object = tables.object(&entry.0, entry.1).unwrap().unwrap();
                let package_obj = object.as_package();
                let start = Instant::now();
                let mut modules = 0u64;
                for bytes in package_obj.modules.values() {
                    move_binary_format::CompiledModule::deserialize_with_defaults(bytes).unwrap();
                    modules += 1;
                }
                let deserialize_ns = start.elapsed().as_nanos() as u64;
                writeln!(
                    out,
                    "{}",
                    serde_json::json!({
                        "kind": "package", "pass": pass, "object_id": id.to_string(),
                        "fetch_ns": t.fetch_ns, "bytes": t.bytes,
                        "modules": modules, "deserialize_ns": deserialize_ns,
                    })
                )
                .unwrap();
                println!(
                    "{pass}: package {id}: fetch {} ns, deserialize {modules} modules {deserialize_ns} ns",
                    t.fetch_ns
                );
            }
        }
    }
    println!("rows written to {}", out_path.display());
}

#[tokio::main]
async fn main() {
    match Cli::parse().cmd {
        Cmd::Populate {
            db_path,
            num_objects,
            object_size,
            seed,
            with_framework_packages,
        } => populate(
            db_path,
            num_objects,
            object_size,
            seed,
            with_framework_packages,
        ),
        Cmd::Measure {
            db_path,
            num_objects,
            seed,
            sample,
            out,
            packages,
        } => measure(db_path, num_objects, seed, sample, out, packages),
    }
}

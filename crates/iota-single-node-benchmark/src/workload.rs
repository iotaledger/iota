// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, sync::Arc};

use iota_sdk_types::ObjectId;
use iota_test_transaction_builder::PublishData;

use crate::{
    benchmark_context::BenchmarkContext,
    command::{PtbParams, WorkloadKind, load_mixture},
    tx_generator::{MixedTxGenerator, MoveTxGenerator, PackagePublishTxGenerator, TxGenerator},
};

/// Generate and publish `count` standalone packages whose only content is a
/// constant-pool pad and a function to call, for the package-load sweeps.
async fn prepare_generated_packages(
    ctx: &mut BenchmarkContext,
    count: u64,
    pad_bytes: u64,
) -> Vec<ObjectId> {
    let mut ids = Vec::new();
    if count == 0 {
        return ids;
    }
    let framework = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../iota-framework/packages/iota-framework")
        .canonicalize()
        .expect("framework package path");
    let base =
        std::env::temp_dir().join(format!("bench-generated-packages-{}", std::process::id()));
    for i in 0..count {
        let dir = base.join(format!("pkg{i}"));
        std::fs::create_dir_all(dir.join("sources")).unwrap();
        // Distinct padding per package so module bytes differ.
        let pad_hex: String = format!("{:02x}", (i % 251) + 1).repeat(pad_bytes as usize);
        let move_toml = format!(
            concat!(
                "[package]\nname = \"Generated{}\"\nversion = \"0.0.1\"\nedition = \"2024\"\n\n",
                "[dependencies]\nIota = {{ local = \"{}\" }}\n\n",
                "[addresses]\ngenerated = \"0x0\"\n"
            ),
            i,
            framework.display()
        );
        std::fs::write(dir.join("Move.toml"), move_toml).unwrap();
        let module_source = format!(
            concat!(
                "module generated::generated {{\n",
                "    const PAD: vector<u8> = x\"{}\";\n\n",
                // load_me must not touch PAD: returning it would put the
                // package's size on the operand stack and confound the
                // package-load cost with stack-byte flow.
                "    public fun load_me(): u64 {{\n",
                "        42\n",
                "    }}\n\n",
                "    public fun pad(): vector<u8> {{\n",
                "        PAD\n",
                "    }}\n",
                "}}\n"
            ),
            pad_hex
        );
        std::fs::write(dir.join("sources").join("generated.move"), module_source).unwrap();
        let package = ctx.publish_package(PublishData::Source(dir, false)).await;
        ids.push(package.object_id);
    }
    ids
}

async fn prepare_ptb_fixtures(
    ctx: &mut BenchmarkContext,
    move_package: ObjectId,
    params: &PtbParams,
) -> (
    std::collections::HashMap<iota_sdk_types::Address, Vec<iota_sdk_types::ObjectReference>>,
    Vec<ObjectId>,
) {
    let owned_objects = ctx
        .preparing_owned_objects(
            move_package,
            params.num_mutations + params.num_burns,
            params.owned_object_size,
        )
        .await;
    let package_count = if params.num_packages_called > 0 {
        params
            .generated_package_count
            .max(params.num_packages_called)
    } else {
        0
    };
    let generated_packages =
        prepare_generated_packages(ctx, package_count, params.generated_package_bytes).await;
    (owned_objects, generated_packages)
}

#[derive(Clone)]
pub struct Workload {
    pub tx_count: u64,
    pub workload_kind: WorkloadKind,
}

impl Workload {
    pub fn new(tx_count: u64, workload_kind: WorkloadKind) -> Self {
        Self {
            tx_count,
            workload_kind,
        }
    }

    pub(crate) fn num_accounts(&self) -> u64 {
        self.tx_count
    }

    pub(crate) fn gas_object_num_per_account(&self) -> u64 {
        self.workload_kind.gas_object_num_per_account()
    }

    pub(crate) async fn create_tx_generator(
        &self,
        ctx: &mut BenchmarkContext,
    ) -> Arc<dyn TxGenerator> {
        match &self.workload_kind {
            WorkloadKind::PTB(params) => {
                let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                path.extend(["move_package"]);
                let move_package = ctx.publish_package(PublishData::Source(path, false)).await;
                let root_objects = ctx
                    .preparing_dynamic_fields(
                        move_package.object_id,
                        params.num_dynamic_fields,
                        params.dynamic_field_size,
                    )
                    .await;
                let shared_objects = ctx
                    .prepare_shared_objects(move_package.object_id, params.num_shared_objects)
                    .await;
                let (owned_objects, generated_packages) =
                    prepare_ptb_fixtures(ctx, move_package.object_id, params).await;
                Arc::new(MoveTxGenerator::new(
                    move_package.object_id,
                    params.clone(),
                    root_objects,
                    shared_objects,
                    owned_objects,
                    generated_packages,
                ))
            }
            WorkloadKind::Mixed { spec_file } => {
                let entries = load_mixture(spec_file);
                let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                path.extend(["move_package"]);
                let move_package = ctx.publish_package(PublishData::Source(path, false)).await;
                // One set of root objects sized for the largest consumer;
                // shapes that declare no dynamic fields get no root, so their
                // transactions stay free of dynamic-field reads.
                let max_fields = entries
                    .iter()
                    .map(|e| e.params.num_dynamic_fields)
                    .max()
                    .unwrap_or(0);
                let max_field_size = entries
                    .iter()
                    .map(|e| e.params.dynamic_field_size)
                    .max()
                    .unwrap_or(0);
                let root_objects = ctx
                    .preparing_dynamic_fields(move_package.object_id, max_fields, max_field_size)
                    .await;
                // Fixture pools sized for the largest consumer in the mixture.
                let max_owned = entries
                    .iter()
                    .map(|e| e.params.num_mutations + e.params.num_burns)
                    .max()
                    .unwrap_or(0);
                let max_owned_size = entries
                    .iter()
                    .map(|e| e.params.owned_object_size)
                    .max()
                    .unwrap_or(64);
                let owned_objects = ctx
                    .preparing_owned_objects(move_package.object_id, max_owned, max_owned_size)
                    .await;
                let max_packages = entries
                    .iter()
                    .map(|e| {
                        if e.params.num_packages_called > 0 {
                            e.params
                                .generated_package_count
                                .max(e.params.num_packages_called)
                        } else {
                            0
                        }
                    })
                    .max()
                    .unwrap_or(0);
                let max_package_bytes = entries
                    .iter()
                    .map(|e| e.params.generated_package_bytes)
                    .max()
                    .unwrap_or(4096);
                let generated_packages =
                    prepare_generated_packages(ctx, max_packages, max_package_bytes).await;
                let weighted = entries
                    .into_iter()
                    .map(|entry| {
                        let roots = if entry.params.num_dynamic_fields > 0 {
                            root_objects.clone()
                        } else {
                            Default::default()
                        };
                        let owned = if entry.params.num_mutations + entry.params.num_burns > 0 {
                            owned_objects.clone()
                        } else {
                            Default::default()
                        };
                        (
                            entry.weight,
                            MoveTxGenerator::new(
                                move_package.object_id,
                                entry.params,
                                roots,
                                vec![],
                                owned,
                                generated_packages.clone(),
                            ),
                        )
                    })
                    .collect();
                Arc::new(MixedTxGenerator::new(weighted))
            }
            WorkloadKind::Publish {
                manifest_file: manifest_path,
            } => Arc::new(PackagePublishTxGenerator::new(ctx, manifest_path.clone()).await),
        }
    }
}

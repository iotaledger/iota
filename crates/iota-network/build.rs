// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    env,
    path::{Path, PathBuf},
};

use tonic_build::manual::{Builder, Method, Service};

type Result<T> = ::std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() -> Result<()> {
    println!("cargo::rustc-check-cfg=cfg(msim)");
    println!("cargo::rustc-check-cfg=cfg(fail_points)");

    let out_dir = if env::var("DUMP_GENERATED_GRPC").is_ok() {
        PathBuf::from("")
    } else {
        PathBuf::from(env::var("OUT_DIR")?)
    };

    let codec_path = "iota_network_stack::codec::BcsCodec";

    let validator_service = Service::builder()
        .name("Validator")
        .package("iota.validator")
        .comment("The Validator interface")
        .method(
            Method::builder()
                .name("transaction")
                .route_name("Transaction")
                .input_type("iota_types::transaction::Transaction")
                .output_type("iota_types::messages_grpc::HandleTransactionResponse")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            Method::builder()
                .name("handle_certificate_v1")
                .route_name("CertifiedTransactionV1")
                .input_type("iota_types::messages_grpc::HandleCertificateRequestV1")
                .output_type("iota_types::messages_grpc::HandleCertificateResponseV1")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            Method::builder()
                .name("handle_soft_bundle_certificates_v1")
                .route_name("SoftBundleCertifiedTransactionsV1")
                .input_type("iota_types::messages_grpc::HandleSoftBundleCertificatesRequestV1")
                .output_type("iota_types::messages_grpc::HandleSoftBundleCertificatesResponseV1")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            Method::builder()
                .name("submit_certificate")
                .route_name("SubmitCertificate")
                .input_type("iota_types::transaction::CertifiedTransaction")
                .output_type("iota_types::messages_grpc::SubmitCertificateResponse")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            Method::builder()
                .name("object_info")
                .route_name("ObjectInfo")
                .input_type("iota_types::messages_grpc::ObjectInfoRequest")
                .output_type("iota_types::messages_grpc::ObjectInfoResponse")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            Method::builder()
                .name("transaction_info")
                .route_name("TransactionInfo")
                .input_type("iota_types::messages_grpc::TransactionInfoRequest")
                .output_type("iota_types::messages_grpc::TransactionInfoResponse")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            Method::builder()
                .name("checkpoint")
                .route_name("Checkpoint")
                .input_type("iota_types::messages_checkpoint::CheckpointRequest")
                .output_type("iota_types::messages_checkpoint::CheckpointResponse")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            Method::builder()
                .name("get_system_state_object")
                .route_name("GetSystemStateObject")
                .input_type("iota_types::messages_grpc::SystemStateRequest")
                .output_type("iota_types::iota_system_state::IotaSystemState")
                .codec_path(codec_path)
                .build(),
        )
        .build();

    Builder::new()
        .out_dir(&out_dir)
        .compile(&[validator_service]);

    build_anemo_services(&out_dir);

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DUMP_GENERATED_GRPC");

    Ok(())
}

fn build_anemo_services(out_dir: &Path) {
    let codec_path = "iota_network_stack::codec::anemo::BcsSnappyCodec";

    let discovery = anemo_build::manual::Service::builder()
        .name("Discovery")
        .package("iota")
        .method(
            anemo_build::manual::Method::builder()
                .name("get_known_peers")
                .route_name("GetKnownPeers")
                .request_type("()")
                .response_type("crate::discovery::GetKnownPeersResponse")
                .codec_path(codec_path)
                .build(),
        )
        .build();

    let state_sync = anemo_build::manual::Service::builder()
        .name("StateSync")
        .package("iota")
        .method(
            anemo_build::manual::Method::builder()
                .name("push_checkpoint_summary")
                .route_name("PushCheckpointSummary")
                .request_type("iota_types::messages_checkpoint::CertifiedCheckpointSummary")
                .response_type("()")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            anemo_build::manual::Method::builder()
                .name("get_checkpoint_summary")
                .route_name("GetCheckpointSummary")
                .request_type("crate::state_sync::GetCheckpointSummaryRequest")
                .response_type(
                    "Option<iota_types::messages_checkpoint::CertifiedCheckpointSummary>",
                )
                .codec_path(codec_path)
                .build(),
        )
        .method(
            anemo_build::manual::Method::builder()
                .name("get_checkpoint_contents")
                .route_name("GetCheckpointContents")
                .request_type("iota_types::messages_checkpoint::CheckpointContentsDigest")
                .response_type("Option<iota_types::messages_checkpoint::FullCheckpointContents>")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            anemo_build::manual::Method::builder()
                .name("get_checkpoint_availability")
                .route_name("GetCheckpointAvailability")
                .request_type("()")
                .response_type("crate::state_sync::GetCheckpointAvailabilityResponse")
                .codec_path(codec_path)
                .build(),
        )
        .build();

    let randomness = anemo_build::manual::Service::builder()
        .name("Randomness")
        .package("iota")
        .method(
            anemo_build::manual::Method::builder()
                .name("send_signatures")
                .route_name("SendSignatures")
                .request_type("crate::randomness::SendSignaturesRequest")
                .response_type("()")
                .codec_path(codec_path)
                .build(),
        )
        .build();

    anemo_build::manual::Builder::new()
        .out_dir(out_dir)
        .compile(&[discovery, state_sync, randomness]);
}

// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    env,
    path::{Path, PathBuf},
};

use tonic_build::{
    Method as MethodTrait, Service as ServiceTrait,
    manual::{Builder, Method, Service},
};

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
        .method(
            Method::builder()
                .name("handle_capability_notification_v1")
                .route_name("CapabilityNotificationV1")
                .input_type("iota_types::messages_grpc::HandleCapabilityNotificationRequestV1")
                .output_type("iota_types::messages_grpc::HandleCapabilityNotificationResponseV1")
                .codec_path(codec_path)
                .build(),
        )
        .build();

    // Generate the method path constants before compiling the service.
    generate_method_paths(&validator_service, &out_dir);

    Builder::new()
        .out_dir(&out_dir)
        .compile(&[validator_service]);

    build_anemo_services(&out_dir);

    build_proto_services(&out_dir)?;

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=proto/");
    println!("cargo:rerun-if-env-changed=DUMP_GENERATED_GRPC");

    Ok(())
}

fn build_proto_services(out_dir: &Path) -> Result<()> {
    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .bytes(".")
        .out_dir(out_dir)
        .compile_protos(
            &["proto/validator_v2.proto", "proto/validator_peer.proto"],
            &["proto/"],
        )?;

    Ok(())
}

/// Generate a Rust source file containing a constant array of all known gRPC
/// method paths for the given service (plus the standard health check path).
///
/// This keeps `VALIDATOR_METHOD_PATHS` in sync with the service definition
/// automatically — no manual updates needed when methods are added or removed.
fn generate_method_paths(service: &Service, out_dir: &Path) {
    let package = ServiceTrait::package(service);
    let service_name = ServiceTrait::name(service);

    let mut paths = vec![
        // The health check service is always registered by ServerBuilder.
        "\"/grpc.health.v1.Health/Check\"".to_string(),
    ];

    for method in ServiceTrait::methods(service) {
        paths.push(format!(
            "\"/{}.{}/{}\"",
            package,
            service_name,
            MethodTrait::identifier(method)
        ));
    }

    let count = paths.len();
    let entries = paths.join(",\n    ");
    let code = format!(
        "/// Known gRPC method paths for the {service_name} service and the health\n\
         /// check service that is always registered alongside it.\n\
         ///\n\
         /// Auto-generated from the service definition in `build.rs`.\n\
         /// Do not edit manually.\n\
         pub const VALIDATOR_METHOD_PATHS: [&str; {count}] = [\n    {entries}\n];\n"
    );

    std::fs::write(out_dir.join("validator_method_paths.rs"), code).unwrap();
}

fn build_anemo_services(out_dir: &Path) {
    let codec_path = "iota_network_stack::codec::anemo::BcsSnappyCodec";

    let discovery = anemo_build::manual::Service::builder()
        .name("Discovery")
        .package("iota")
        .method(
            anemo_build::manual::Method::builder()
                .name("get_known_peers_v2")
                .route_name("GetKnownPeersV2")
                .request_type("()")
                .response_type("crate::discovery::GetKnownPeersResponseV2")
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
        .method(
            anemo_build::manual::Method::builder()
                .name("exchange_state_sync_handshake")
                .route_name("ExchangeStateSyncHandshake")
                .request_type("crate::state_sync::StateSyncHandshake")
                .response_type("crate::state_sync::StateSyncHandshake")
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

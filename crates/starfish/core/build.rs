// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    env,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

// Build script to generate RPC stubs.
fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    build_tonic_services(&out_dir);

    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}

fn build_tonic_services(out_dir: &Path) {
    let codec_path = "tonic_prost::ProstCodec";

    let service = tonic_build::manual::Service::builder()
        .name("ConsensusService")
        .package("consensus")
        .comment("Consensus authority interface")
        .method(
            tonic_build::manual::Method::builder()
                .name("subscribe_block_bundles")
                .route_name("SubscribeBlockBundles")
                .input_type("crate::network::tonic_network::SubscribeBlockBundlesRequest")
                .output_type("crate::network::tonic_network::SubscribeBlockBundlesResponse")
                .codec_path(codec_path)
                .server_streaming()
                .client_streaming()
                .build(),
        )
        .method(
            tonic_build::manual::Method::builder()
                .name("fetch_block_headers")
                .route_name("FetchBlockHeaders")
                .input_type("crate::network::tonic_network::FetchBlockHeadersRequest")
                .output_type("crate::network::tonic_network::FetchBlockHeadersResponse")
                .codec_path(codec_path)
                .server_streaming()
                .build(),
        )
        .method(
            tonic_build::manual::Method::builder()
                .name("fetch_commits")
                .route_name("FetchCommits")
                .input_type("crate::network::tonic_network::FetchCommitsRequest")
                .output_type("crate::network::tonic_network::FetchCommitsResponse")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            tonic_build::manual::Method::builder()
                .name("fetch_commits_and_transactions")
                .route_name("FetchCommitsAndTransactions")
                .input_type("crate::network::tonic_network::FetchCommitsAndTransactionsRequest")
                .output_type("crate::network::tonic_network::FetchCommitsAndTransactionsResponse")
                .codec_path(codec_path)
                .server_streaming()
                .build(),
        )
        .method(
            tonic_build::manual::Method::builder()
                .name("fetch_latest_block_headers")
                .route_name("FetchLatestBlockHeaders")
                .input_type("crate::network::tonic_network::FetchLatestBlockHeadersRequest")
                .output_type("crate::network::tonic_network::FetchLatestBlockHeadersResponse")
                .codec_path(codec_path)
                .server_streaming()
                .build(),
        )
        .method(
            tonic_build::manual::Method::builder()
                .name("get_latest_rounds")
                .route_name("GetLatestRounds")
                .input_type("crate::network::tonic_network::GetLatestRoundsRequest")
                .output_type("crate::network::tonic_network::GetLatestRoundsResponse")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            tonic_build::manual::Method::builder()
                .name("fetch_transactions")
                .route_name("FetchTransactions")
                .input_type("crate::network::tonic_network::FetchTransactionsRequest")
                .output_type("crate::network::tonic_network::FetchTransactionsResponse")
                .codec_path(codec_path)
                .server_streaming()
                .build(),
        )
        .build();

    tonic_build::manual::Builder::new()
        .out_dir(out_dir)
        .compile(&[service]);
}

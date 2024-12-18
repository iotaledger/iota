// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use rand::Rng;

use crate::Benchmark;

pub struct Server;

#[anemo::async_trait]
impl Benchmark for Server {
    async fn send_bytes(
        &self,
        _request: anemo::Request<Vec<u8>>,
    ) -> Result<anemo::Response<()>, anemo::rpc::Status> {
        Ok(anemo::Response::new(()))
    }

    async fn request_bytes(
        &self,
        request: anemo::Request<u32>,
    ) -> Result<anemo::Response<Vec<u8>>, anemo::rpc::Status> {
        let rng = rand::thread_rng();
        Ok(anemo::Response::new(
            rng.sample_iter(rand::distributions::Standard)
                .take(request.into_inner() as usize)
                .collect(),
        ))
    }
}

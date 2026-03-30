// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_network::{
    api::{SubmitTxRequest, TxDigest, TxStatus, ValidatorV2},
    tonic::{Request, Response, Status},
};
use tokio_stream::wrappers::ReceiverStream;

use crate::authority_server::ValidatorService;

#[async_trait::async_trait]
impl ValidatorV2 for ValidatorService {
    type SubmitTxStream = ReceiverStream<Result<TxStatus, Status>>;

    async fn submit_tx(
        &self,
        _request: Request<SubmitTxRequest>,
    ) -> Result<Response<Self::SubmitTxStream>, Status> {
        todo!()
    }

    type GetTxStatusStream = ReceiverStream<Result<TxStatus, Status>>;

    async fn get_tx_status(
        &self,
        _request: Request<TxDigest>,
    ) -> Result<Response<Self::GetTxStatusStream>, Status> {
        todo!()
    }
}

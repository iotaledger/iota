// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod validator {
    include!(concat!(env!("OUT_DIR"), "/iota.validator.Validator.rs"));
}

pub use validator::{
    validator_client::ValidatorClient,
    validator_server::{Validator, ValidatorServer},
};

include!(concat!(env!("OUT_DIR"), "/validator_method_paths.rs"));

mod validator_v2 {
    tonic::include_proto!("iota.validator.v2");
}

pub use validator_v2::{
    ExecutedStatus, ExpiredStatus, RejectedStatus, StatusDetail, SubmitTxRequest, SubmittedStatus,
    TxDigest, TxStatus, status_detail,
    validator_v2_client::ValidatorV2Client,
    validator_v2_server::{ValidatorV2, ValidatorV2Server},
};

mod validator_peer {
    tonic::include_proto!("iota.validator.peer");
}

pub use validator_peer::{
    GetCheckpointRequest, GetCheckpointResponse,
    validator_peer_client::ValidatorPeerClient,
    validator_peer_server::{ValidatorPeer, ValidatorPeerServer},
};

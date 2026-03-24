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

mod validator_v2 {
    tonic::include_proto!("iota.validator.v2");
}

pub use validator_v2::{
    Status, SubmitTxRequest, TxDigest, TxStatus, validator_v2_client::ValidatorV2Client,
    validator_v2_server::{ValidatorV2, ValidatorV2Server},
};

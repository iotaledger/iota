// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;

use super::base64::Base64;

/// The credentials related fields associated with a validator.
#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub(crate) struct ValidatorCredentials {
    pub protocol_pub_key: Option<Base64>,
    pub network_pub_key: Option<Base64>,
    pub worker_pub_key: Option<Base64>,
    pub proof_of_possession: Option<Base64>,
    pub net_address: Option<String>,
    pub p2p_address: Option<String>,
    pub primary_address: Option<String>,
    pub worker_address: Option<String>,
}

// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::authority_client::NetworkAuthorityClient;

pub trait ValidatorPeerAPI {}

impl ValidatorPeerAPI for NetworkAuthorityClient {}

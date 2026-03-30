// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::authority_client::NetworkAuthorityClient;

pub trait ValidatorV2API {}

impl ValidatorV2API for NetworkAuthorityClient {}

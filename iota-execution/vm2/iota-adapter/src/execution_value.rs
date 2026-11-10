// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::storage::{ChildObjectResolver, Storage};

/// Interface with the store necessary to execute a programmable transaction
pub trait ExecutionState: Storage + ChildObjectResolver {}

impl<T> ExecutionState for T where T: Storage + ChildObjectResolver {}

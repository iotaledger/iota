// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The package machine. Loads the package from the store first, then decides
//! visibility from the row at its version and the sync-ahead record, for both
//! the input loader and the deny check's package store.

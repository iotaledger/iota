// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use lsp_server::Connection;

use crate::symbols::Symbols;

/// The context within which the language server is running.
pub struct Context {
    /// The connection with the language server's client.
    pub connection: Connection,
    /// Symbolication information
    pub symbols: Arc<Mutex<BTreeMap<PathBuf, Symbols>>>,
    /// Are inlay type hints enabled?
    pub inlay_type_hints: bool,
    /// Are param type hints enabled?
    pub inlay_param_hints: bool,
}

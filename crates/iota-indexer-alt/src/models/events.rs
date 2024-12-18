// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;
use iota_field_count::FieldCount;

use crate::schema::{ev_emit_mod, ev_struct_inst};

#[derive(Insertable, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, FieldCount)]
#[diesel(table_name = ev_emit_mod)]
pub struct StoredEvEmitMod {
    pub package: Vec<u8>,
    pub module: String,
    pub tx_sequence_number: i64,
    pub sender: Vec<u8>,
}

#[derive(Insertable, Debug, Clone, Eq, PartialEq, Ord, PartialOrd, FieldCount)]
#[diesel(table_name = ev_struct_inst)]
pub struct StoredEvStructInst {
    pub package: Vec<u8>,
    pub module: String,
    pub name: String,
    pub instantiation: Vec<u8>,
    pub tx_sequence_number: i64,
    pub sender: Vec<u8>,
}

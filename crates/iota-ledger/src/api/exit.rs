// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use ledger_transport::APDUCommand;

use crate::{
    Transport,
    api::{constants, errors, helpers},
};

pub fn exec<T: Transport>(transport: &T) -> Result<(), errors::LedgerError> {
    let cmd = APDUCommand {
        cla: constants::APDU_CLA,
        ins: constants::APDUInstructions::Exit as u8,
        p1: 0,
        p2: 0,
        data: Vec::new(),
    };
    helpers::exec::<T, ()>(transport, cmd)
}

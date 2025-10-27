// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use ledger_transport::APDUCommand;

use crate::{
    Transport,
    api::{bolos, errors, helpers},
};

pub fn exec<T: Transport>(transport: &T) -> Result<(), errors::LedgerError> {
    let cmd = APDUCommand {
        cla: bolos::APDU_CLA_B0,
        ins: bolos::APDUInstructions::AppExitB0 as u8,
        p1: bolos::APDU_P1,
        p2: bolos::APDU_P2,
        data: Vec::new(),
    };
    helpers::exec::<T, ()>(transport, cmd)
}

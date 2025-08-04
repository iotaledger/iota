// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use ledger_transport::APDUCommand;

use crate::{
    Transport,
    api::{
        bolos, errors, helpers,
        packable::{Error as PackableError, Read, Unpackable},
    },
};
// dashboard:
// HID => b001000000
// HID <= 0105|424f4c4f53|05|322e302e30|9000
// B O L O S      2 . 0 . 0
//
// "IOTA"
// HID => b001000000
// HID <= 0104|494f5441|05|302e372e30|0102|9000
// I O T A      0 . 7 . 0

#[expect(dead_code)]
pub struct Response {
    pub app: String,
    pub version: String,
}

impl Unpackable for Response {
    fn unpack<R: Read>(buf: &mut R) -> Result<Self, PackableError>
    where
        Self: Sized,
    {
        // format always 0x01 but don't insist on it
        let _format_id = u8::unpack(buf)?;

        let app = String::unpack(buf)?;
        let version = String::unpack(buf)?;

        // consume all extra bytes (nano x <-> nano s compatibility!)
        while u8::unpack(buf).is_ok() {
            // NOP
        }

        Ok(Self { app, version })
    }
}

pub fn exec<T: Transport>(transport: &T) -> Result<Response, errors::LedgerError> {
    let cmd = APDUCommand {
        cla: bolos::APDU_CLA_B0,
        ins: bolos::APDUInstructions::GetAppVersionB0 as u8,
        p1: bolos::APDU_P1,
        p2: bolos::APDU_P2,
        data: Vec::new(),
    };
    helpers::exec::<T, Response>(transport, cmd)
}

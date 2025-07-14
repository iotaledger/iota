// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub enum APDUInstructions {
    GetVersion = 0x00,
    VerifyAddress = 0x01,
    GetPublicKey = 0x02,
    SignTransaction = 0x03,
    Exit = 0xff,
}

pub(crate) const APDU_CLA: u8 = 0x00;
pub(crate) const APDU_P1: u8 = 0x00;
pub(crate) const APDU_P2: u8 = 0x00;

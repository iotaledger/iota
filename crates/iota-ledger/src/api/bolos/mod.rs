// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub(crate) mod app_exit;
pub(crate) mod app_get_name;
pub(crate) mod app_open;

pub(crate) const APDU_CLA_B0: u8 = 0xb0;
pub(crate) const APDU_CLA_E0: u8 = 0xe0;
pub(crate) const APDU_P1: u8 = 0x00;
pub(crate) const APDU_P2: u8 = 0x00;

pub(crate) enum APDUInstructions {
    GetAppVersionB0 = 0x01,
    AppExitB0 = 0xa7,
    OpenAppE0 = 0xd8,
}

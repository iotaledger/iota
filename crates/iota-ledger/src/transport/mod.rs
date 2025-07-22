// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use ledger_transport::{APDUAnswer, APDUCommand};
pub use ledger_transport_hid::LedgerHIDError;
use ledger_transport_hid::TransportNativeHID;

use crate::LedgerError;
mod tcp;
pub use hidapi::HidError;
pub use tcp::LedgerTCPError;
use tcp::TransportTCP;

#[allow(clippy::upper_case_acronyms)]
pub(crate) enum LedgerTransport {
    Simulator(TransportTCP),
    NativeHID(TransportNativeHID),
}

pub(crate) trait Transport {
    fn exchange(
        &self,
        apdu_command: &APDUCommand<Vec<u8>>,
    ) -> Result<APDUAnswer<Vec<u8>>, LedgerError>;
}

impl LedgerTransport {
    pub(crate) fn new_simulator() -> Result<LedgerTransport, LedgerError> {
        Ok(LedgerTransport::Simulator(TransportTCP::new(
            "127.0.0.1",
            9999,
        )))
    }

    pub(crate) fn new_native_hid() -> Result<LedgerTransport, LedgerError> {
        let api = hidapi::HidApi::new()?;
        Ok(LedgerTransport::NativeHID(
            TransportNativeHID::new(&api).map_err(|e| match e {
                LedgerHIDError::DeviceNotFound => LedgerError::DeviceNotFound,
                _ => e.into(),
            })?,
        ))
    }
}

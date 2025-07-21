// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use hex::ToHex;
use ledger_transport::{APDUAnswer, APDUCommand};
pub use ledger_transport_hid::LedgerHIDError;
use ledger_transport_hid::TransportNativeHID;
use tracing::debug;

use crate::LedgerError;
mod tcp;
pub use hidapi::HidError;
pub use tcp::LedgerTCPError;
use tcp::TransportTCP;

pub(crate) struct Transport {
    transport: LedgerTransport,
}

#[allow(clippy::upper_case_acronyms)]
pub(crate) enum LedgerTransport {
    Simulator(TransportTCP),
    NativeHID(TransportNativeHID),
}

impl Transport {
    pub(crate) fn new_simulator() -> Result<Transport, LedgerError> {
        Ok(Transport {
            transport: create_tcp_transport()?,
        })
    }

    pub(crate) fn new_native_hid() -> Result<Transport, LedgerError> {
        Ok(Transport {
            transport: create_hid_transport()?,
        })
    }

    pub(crate) fn exchange(
        &self,
        apdu_command: &APDUCommand<Vec<u8>>,
    ) -> Result<APDUAnswer<Vec<u8>>, LedgerError> {
        debug!(
            "Exchanging APDU command: {}",
            apdu_command.serialize().encode_hex::<String>()
        );
        match &self.transport {
            LedgerTransport::Simulator(tcp) => Ok(tcp.exchange(apdu_command)?),
            LedgerTransport::NativeHID(hid) => Ok(hid.exchange(apdu_command)?),
        }
    }

    pub(crate) fn is_simulator(&self) -> bool {
        matches!(&self.transport, LedgerTransport::Simulator(_))
    }

    pub(crate) fn recreate(&mut self) -> Result<(), LedgerError> {
        match &self.transport {
            LedgerTransport::Simulator(_) => {
                self.transport = create_tcp_transport()?;
            }
            LedgerTransport::NativeHID(_) => {
                self.transport = create_hid_transport()?;
            }
        }
        Ok(())
    }
}

fn create_tcp_transport() -> Result<LedgerTransport, LedgerError> {
    Ok(LedgerTransport::Simulator(TransportTCP::new(
        "127.0.0.1",
        9999,
    )))
}

fn create_hid_transport() -> Result<LedgerTransport, LedgerError> {
    let api = hidapi::HidApi::new()?;
    Ok(LedgerTransport::NativeHID(
        TransportNativeHID::new(&api).map_err(|e| match e {
            LedgerHIDError::DeviceNotFound => LedgerError::DeviceNotFound,
            _ => e.into(),
        })?,
    ))
}

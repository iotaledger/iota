// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use hex::ToHex;
use ledger_transport::{APDUAnswer, APDUCommand};
use ledger_transport_hid::{LedgerHIDError, TransportNativeHID};
use tracing::debug;

use crate::LedgerError;

mod tcp;
use tcp::TransportTCP;

#[derive(Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub(crate) enum TransportType {
    TCP,
    NativeHID,
}

pub(crate) struct Transport {
    transport: LedgerTransport,
    type_: TransportType,
}

#[allow(clippy::upper_case_acronyms)]
pub(crate) enum LedgerTransport {
    TCP(TransportTCP),
    NativeHID(TransportNativeHID),
}

impl Transport {
    pub(crate) fn exchange(
        &self,
        apdu_command: &APDUCommand<Vec<u8>>,
    ) -> Result<APDUAnswer<Vec<u8>>, LedgerError> {
        debug!(
            "Exchanging APDU command: {}",
            apdu_command.serialize().encode_hex::<String>()
        );
        match &self.transport {
            LedgerTransport::TCP(t) => t.exchange(apdu_command).map_err(|_| LedgerError::Transport),
            LedgerTransport::NativeHID(h) => {
                h.exchange(apdu_command).map_err(|_| LedgerError::Transport)
            }
        }
    }

    pub(crate) fn recreate(&mut self) -> Result<(), LedgerError> {
        self.transport = create_ledger_transport(self.type_)?;
        Ok(())
    }
}

fn create_ledger_transport(transport_type: TransportType) -> Result<LedgerTransport, LedgerError> {
    let transport = match transport_type {
        TransportType::TCP => LedgerTransport::TCP(TransportTCP::new("127.0.0.1", 9999)),
        TransportType::NativeHID => {
            let api = hidapi::HidApi::new().map_err(|_| LedgerError::Transport)?;
            LedgerTransport::NativeHID(TransportNativeHID::new(&api).map_err(|e| match e {
                LedgerHIDError::DeviceNotFound => LedgerError::DeviceNotFound,
                _ => LedgerError::Transport,
            })?)
        }
    };
    Ok(transport)
}

pub(crate) fn create_transport(transport_type: TransportType) -> Result<Transport, LedgerError> {
    Ok(Transport {
        transport: create_ledger_transport(transport_type)?,
        type_: transport_type,
    })
}

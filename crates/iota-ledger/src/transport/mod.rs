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
pub(crate) enum TransportTypes {
    TCP,
    NativeHID,
}

pub(crate) struct Transport {
    pub transport: LedgerTransport,
}

#[allow(clippy::upper_case_acronyms)]
pub(crate) enum LedgerTransport {
    TCP(TransportTCP),
    NativeHID(TransportNativeHID),
}

impl LedgerTransport {
    pub(crate) fn exchange(
        &self,
        apdu_command: &APDUCommand<Vec<u8>>,
    ) -> Result<APDUAnswer<Vec<u8>>, LedgerError> {
        debug!(
            "Exchanging APDU command: {}",
            apdu_command.serialize().encode_hex::<String>()
        );
        match self {
            LedgerTransport::TCP(t) => t
                .exchange(apdu_command)
                .map_err(|_| LedgerError::TransportError),
            LedgerTransport::NativeHID(h) => h
                .exchange(apdu_command)
                .map_err(|_| LedgerError::TransportError),
        }
    }
}

// only create transport without IOTA specific calls
pub(crate) fn create_transport(transport_type: TransportTypes) -> Result<Transport, LedgerError> {
    let transport = match transport_type {
        TransportTypes::TCP => Transport {
            transport: LedgerTransport::TCP(TransportTCP::new("127.0.0.1", 9999)),
        },
        TransportTypes::NativeHID => {
            let api = hidapi::HidApi::new().map_err(|_| LedgerError::TransportError)?;
            Transport {
                transport: LedgerTransport::NativeHID(TransportNativeHID::new(&api).map_err(
                    |e| match e {
                        LedgerHIDError::DeviceNotFound => LedgerError::DeviceNotFound,
                        _ => LedgerError::TransportError,
                    },
                )?),
            }
        }
    };
    Ok(transport)
}

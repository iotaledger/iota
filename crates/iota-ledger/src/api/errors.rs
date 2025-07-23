// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

pub use crate::transport::{HidError, LedgerHIDError, LedgerTCPError};

/// APDU error codes including standard codes from the ledger SDK
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(u16)]
pub enum APDUErrorCode {
    /// No error
    Ok = 0x9000,
    /// Wrong length
    WrongLength = 0x6700,
    /// Nothing received
    NothingReceived = 0x6982,
    /// User cancelled
    UserCancelled = 0x6985,
    /// Wrong data
    WrongData = 0x6a80,
    /// Function not supported
    FunctionNotSupported = 0x6a81,
    /// File not found
    FileNotFound = 0x6a82,
    /// Record not found
    RecordNotFound = 0x6a83,
    /// Not enough memory space
    NotEnoughMemory = 0x6a84,
    /// Wrong P1 P2
    WrongP1P2 = 0x6a86,
    /// Unknown error
    Unknown = 0x6d00,
    /// Bad class
    BadCla = 0x6e00,
    /// Bad instruction
    BadIns = 0x6e01,
    /// Bad P1 P2 parameters
    BadP1P2 = 0x6e02,
    /// Bad length
    BadLen = 0x6e03,
    /// Device panic
    Panic = 0xe000,
    /// Device locked
    DeviceLocked = 0x5515,
    /// User denied the request
    UserDenied = 0x5501,
}

impl TryFrom<u16> for APDUErrorCode {
    type Error = ();

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x9000 => Ok(APDUErrorCode::Ok),
            0x6700 => Ok(APDUErrorCode::WrongLength),
            0x6982 => Ok(APDUErrorCode::NothingReceived),
            0x6985 => Ok(APDUErrorCode::UserCancelled),
            0x6a80 => Ok(APDUErrorCode::WrongData),
            0x6a81 => Ok(APDUErrorCode::FunctionNotSupported),
            0x6a82 => Ok(APDUErrorCode::FileNotFound),
            0x6a83 => Ok(APDUErrorCode::RecordNotFound),
            0x6a84 => Ok(APDUErrorCode::NotEnoughMemory),
            0x6a86 => Ok(APDUErrorCode::WrongP1P2),
            0x6d00 => Ok(APDUErrorCode::Unknown),
            0x6e00 => Ok(APDUErrorCode::BadCla),
            0x6e01 => Ok(APDUErrorCode::BadIns),
            0x6e02 => Ok(APDUErrorCode::BadP1P2),
            0x6e03 => Ok(APDUErrorCode::BadLen),
            0xe000 => Ok(APDUErrorCode::Panic),
            0x5515 => Ok(APDUErrorCode::DeviceLocked),
            0x5501 => Ok(APDUErrorCode::UserDenied),
            _ => Err(()),
        }
    }
}

#[derive(PartialEq, Debug)]
#[repr(u8)]
pub enum SyscallError {
    InvalidParameter = 2,
    Overflow,
    Security,
    InvalidCrc,
    InvalidChecksum,
    InvalidCounter,
    NotSupported,
    InvalidState,
    Timeout,
    Unspecified,
}

#[derive(Error, Debug)]
pub enum LedgerError {
    #[error(
        "Address mismatch - connect the correct Ledger device or select the correct bip32 path"
    )]
    AddressMismatch,

    #[error("Device not ready - ensure the IOTA app is open on the Ledger device")]
    DeviceNotReady,

    #[error("Device not found - connect the Ledger device")]
    DeviceNotFound,

    #[error("Device locked - unlock the Ledger device")]
    DeviceLocked,

    #[error("User refused the operation")]
    UserRefused,

    #[error("Device panic")]
    DevicePanic,

    #[error("App not found - ensure the IOTA app is installed on the Ledger device")]
    AppNotFound,

    #[error("Syscall error: {0:?}")]
    Syscall(SyscallError),

    #[error("APDU error: {0:?}")]
    APDUError(APDUErrorCode),

    #[error("Unknown APDU error {0:?}")]
    UnknownAPDUError(u16),

    #[error("Blocks protocol failed")]
    BlocksProtocolFailed,

    #[error("Hid API error: {0}")]
    HidError(#[from] HidError),

    #[error("HID Transport error: {0}")]
    LedgerHID(#[from] LedgerHIDError),

    #[error("TCP Transport error: {0}")]
    LedgerTCP(#[from] LedgerTCPError),

    #[error("Serialization error")]
    Serialization,
}

impl LedgerError {
    pub fn get_error(rc: u16) -> Option<LedgerError> {
        // First try to match APDU error codes (including ledger SDK standard codes)
        if let Ok(apdu_error) = APDUErrorCode::try_from(rc) {
            return match apdu_error {
                APDUErrorCode::Ok => None, // No error, return None
                APDUErrorCode::DeviceLocked => Some(LedgerError::DeviceLocked),
                APDUErrorCode::Panic => Some(LedgerError::DevicePanic),
                APDUErrorCode::UserCancelled | APDUErrorCode::UserDenied => {
                    Some(LedgerError::UserRefused)
                }
                APDUErrorCode::BadCla | APDUErrorCode::BadIns | APDUErrorCode::BadP1P2 => {
                    Some(LedgerError::DeviceNotReady)
                }
                _ => Some(LedgerError::APDUError(apdu_error)),
            };
        }

        // Handle syscall errors range in the APDU error codes
        let e = match rc {
            rc if (0x6800..=0x680b).contains(&rc) => {
                let value = (rc - 0x6800) as u8;
                let syscall_error = match value {
                    2 => SyscallError::InvalidParameter,
                    3 => SyscallError::Overflow,
                    4 => SyscallError::Security,
                    5 => SyscallError::InvalidCrc,
                    6 => SyscallError::InvalidChecksum,
                    7 => SyscallError::InvalidCounter,
                    8 => SyscallError::NotSupported,
                    9 => SyscallError::InvalidState,
                    10 => SyscallError::Timeout,
                    _ => SyscallError::Unspecified,
                };
                LedgerError::Syscall(syscall_error)
            }
            _ => LedgerError::UnknownAPDUError(rc),
        };
        Some(e)
    }
}

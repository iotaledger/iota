// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LockedDeviceError, StatusCodes } from '@ledgerhq/errors';

import {
    isLedgerTransportStatusError,
    LedgerConnectionFailedError,
    LedgerDeviceNotFoundError,
    LedgerNoTransportMechanismError,
} from '_components';
import { KeystoneSigningCanceledByUserError } from '../components/keystone/keystoneErrors';

/**
 * Helper method for producing user-friendly error messages from Signer operations
 * from SignerWithProvider instances (e.g., signTransaction, getAddress, and so forth)
 */
export function getSignerOperationErrorMessage(error: unknown) {
    return (
        getLedgerConnectionErrorMessage(error) ||
        getKeystoneErrorMessage(error) ||
        getIotaApplicationErrorMessage(error) ||
        (error as Error).message ||
        'Something went wrong.'
    );
}

/**
 * Helper method for producing user-friendly error messages from Ledger connection errors
 */
export function getLedgerConnectionErrorMessage(error: unknown) {
    if (error instanceof LedgerConnectionFailedError) {
        return 'Ledger connection failed. Try again.';
    } else if (error instanceof LedgerNoTransportMechanismError) {
        return "Your browser unfortunately doesn't support USB or HID.";
    } else if (error instanceof LedgerDeviceNotFoundError) {
        return 'Connect your Ledger device and try again.';
    } else if (error instanceof LockedDeviceError) {
        return 'Your device is locked. Unlock it and try again.';
    }
    return null;
}

/**
 * Helper method for producing user-friendly error messages for Keystone
 */
export function getKeystoneErrorMessage(error: unknown) {
    if (error instanceof KeystoneSigningCanceledByUserError) {
        return 'Signing canceled by user.';
    }
    return null;
}

/**
 * Helper method for producing user-friendly error messages from errors that arise from
 * operations on the IOTA Ledger application
 */
export function getIotaApplicationErrorMessage(error: unknown) {
    if (error instanceof LockedDeviceError) {
        return 'Your device is locked. Unlock it and try again.';
    } else if (isLedgerTransportStatusError(error)) {
        if (error.statusCode === StatusCodes.INS_NOT_SUPPORTED) {
            return "Something went wrong. We're working on it!";
        } else if (
            error.statusCode === 0x6e04 ||
            error.statusCode === StatusCodes.CONDITIONS_OF_USE_NOT_SATISFIED
        ) {
            // 0x6e04 is a legacy code that we need to keep support for older versions of the IOTA app (0.9.X)
            // https://github.com/LedgerHQ/ledger-device-rust-sdk/commit/7a16c29b09f1d21916b2d76c9f805580c64ff064
            return 'User rejected the transaction.';
        } else if (error.statusCode === 0x8) {
            // v.0.9.2: 0x8
            return 'Enable Blind Signing in the IOTA app settings on your Ledger device.';
        } else {
            return 'Make sure the IOTA app is open on your device.';
        }
    }
    return null;
}

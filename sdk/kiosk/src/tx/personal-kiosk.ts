// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Transaction, TransactionObjectArgument } from '@iota/iota-sdk/transactions';

import type { ObjectArgument } from '../types/index.js';

export function convertToPersonalTx(
    tx: Transaction,
    kiosk: ObjectArgument,
    kioskOwnerCap: ObjectArgument,
    packageId: string,
): TransactionObjectArgument {
    const personalKioskCap = tx.moveCall({
        target: `${packageId}::personal_kiosk::new`,
        arguments: [tx.object(kiosk), tx.object(kioskOwnerCap)],
    });

    return personalKioskCap;
}

/**
 * Transfers the personal kiosk Cap to the sender.
 */
export function transferPersonalCapTx(
    tx: Transaction,
    personalKioskCap: TransactionObjectArgument,
    packageId: string,
) {
    tx.moveCall({
        target: `${packageId}::personal_kiosk::transfer_to_sender`,
        arguments: [personalKioskCap],
    });
}

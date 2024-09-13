// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    IdentifierRecord,
    StandardConnectFeature,
    StandardDisconnectFeature,
    StandardEventsFeature,
    WalletWithFeatures,
} from '@wallet-standard/core';

import type { IotaSignAndExecuteTransactionBlockFeature } from './iotaSignAndExecuteTransactionBlock.js';
import type { IotaSignMessageFeature } from './iotaSignMessage.js';
import type { IotaSignPersonalMessageFeature } from './iotaSignPersonalMessage.js';
import type { IotaSignTransactionBlockFeature } from './iotaSignTransactionBlock.js';

/**
 * Wallet Standard features that are unique to Iota, and that all Iota wallets are expected to implement.
 */
export type IotaFeatures = IotaSignTransactionBlockFeature &
    IotaSignAndExecuteTransactionBlockFeature &
    IotaSignPersonalMessageFeature &
    // This deprecated feature should be removed once wallets update to the new method:
    Partial<IotaSignMessageFeature>;

export type WalletWithIotaFeatures = WalletWithFeatures<
    StandardConnectFeature &
        StandardEventsFeature &
        IotaFeatures &
        // Disconnect is an optional feature:
        Partial<StandardDisconnectFeature>
>;

/**
 * Represents a wallet with the absolute minimum feature set required to function in the Iota ecosystem.
 */
export type WalletWithRequiredFeatures = WalletWithFeatures<
    MinimallyRequiredFeatures &
        Partial<IotaFeatures> &
        Partial<StandardDisconnectFeature> &
        IdentifierRecord<unknown>
>;

export type MinimallyRequiredFeatures = StandardConnectFeature & StandardEventsFeature;

export * from './iotaSignMessage.js';
export * from './iotaSignTransactionBlock.js';
export * from './iotaSignAndExecuteTransactionBlock.js';
export * from './iotaSignPersonalMessage.js';

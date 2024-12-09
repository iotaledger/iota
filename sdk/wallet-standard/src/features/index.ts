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

import type { IotaReportTransactionEffectsFeature } from './iotaReportTransactionEffects.js';
import type { IotaSignAndExecuteTransactionFeature } from './iotaSignAndExecuteTransaction.js';
import type { IotaSignPersonalMessageFeature } from './iotaSignPersonalMessage.js';
import type { IotaSignTransactionFeature } from './iotaSignTransaction.js';

/**
 * Wallet Standard features that are unique to IOTA, and that all IOTA wallets are expected to implement.
 */
export type IotaFeatures = IotaSignPersonalMessageFeature &
    IotaSignAndExecuteTransactionFeature &
    IotaSignTransactionFeature &
    Partial<IotaReportTransactionEffectsFeature>;

export type IotaWalletFeatures = StandardConnectFeature &
    StandardEventsFeature &
    IotaFeatures &
    // Disconnect is an optional feature:
    Partial<StandardDisconnectFeature>;

export type WalletWithIotaFeatures = WalletWithFeatures<IotaWalletFeatures>;

/**
 * Represents a wallet with the absolute minimum feature set required to function in the IOTA ecosystem.
 */
export type WalletWithRequiredFeatures = WalletWithFeatures<
    MinimallyRequiredFeatures &
        Partial<IotaFeatures> &
        Partial<StandardDisconnectFeature> &
        IdentifierRecord<unknown>
>;

export type MinimallyRequiredFeatures = StandardConnectFeature & StandardEventsFeature;

export * from './iotaSignTransaction.js';
export * from './iotaSignAndExecuteTransaction.js';
export * from './iotaSignPersonalMessage.js';
export * from './iotaReportTransactionEffects.js';

// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletAccount } from '@wallet-standard/core';

/**
 * The latest API version of the signMessage API.
 * @deprecated Wallets can still implement this method for compatibility, but this has been replaced by the `iota:signPersonalMessage` feature
 */
export type IotaSignMessageVersion = '1.0.0';

/**
 * A Wallet Standard feature for signing a personal message, and returning the
 * message bytes that were signed, and message signature.
 *
 * @deprecated Wallets can still implement this method for compatibility, but this has been replaced by the `iota:signPersonalMessage` feature
 */
export type IotaSignMessageFeature = {
    /** Namespace for the feature. */
    'iota:signMessage': {
        /** Version of the feature API. */
        version: IotaSignMessageVersion;
        signMessage: IotaSignMessageMethod;
    };
};

/** @deprecated Wallets can still implement this method for compatibility, but this has been replaced by the `iota:signPersonalMessage` feature */
export type IotaSignMessageMethod = (input: IotaSignMessageInput) => Promise<IotaSignMessageOutput>;

/**
 * Input for signing messages.
 * @deprecated Wallets can still implement this method for compatibility, but this has been replaced by the `iota:signPersonalMessage` feature
 */
export interface IotaSignMessageInput {
    message: Uint8Array;
    account: WalletAccount;
}

/**
 * Output of signing messages.
 * @deprecated Wallets can still implement this method for compatibility, but this has been replaced by the `iota:signPersonalMessage` feature
 */
export interface IotaSignMessageOutput {
    /** Base64 message bytes. */
    messageBytes: string;
    /** Base64 encoded signature */
    signature: string;
}

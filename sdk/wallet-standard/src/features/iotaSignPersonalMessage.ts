// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletAccount } from '@wallet-standard/core';

/** The latest API version of the signPersonalMessage API. */
export type IotaSignPersonalMessageVersion = '1.0.0';

/**
 * A Wallet Standard feature for signing a personal message, and returning the
 * message bytes that were signed, and message signature.
 */
export type IotaSignPersonalMessageFeature = {
    /** Namespace for the feature. */
    'iota:signPersonalMessage': {
        /** Version of the feature API. */
        version: IotaSignPersonalMessageVersion;
        signPersonalMessage: IotaSignPersonalMessageMethod;
    };
};

export type IotaSignPersonalMessageMethod = (
    input: IotaSignPersonalMessageInput,
) => Promise<IotaSignPersonalMessageOutput>;

/** Input for signing personal messages. */
export interface IotaSignPersonalMessageInput {
    message: Uint8Array;
    account: WalletAccount;
}

/** Output of signing personal messages. */
export interface IotaSignPersonalMessageOutput extends SignedPersonalMessage {}

export interface SignedPersonalMessage {
    /** Base64 encoded message bytes */
    bytes: string;
    /** Base64 encoded signature */
    signature: string;
}

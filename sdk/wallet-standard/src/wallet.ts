// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

declare module '@wallet-standard/core' {
    export interface Wallet {
        /**
         * Unique identifier of the Wallet.
         *
         * If not provided, the wallet name will be used as the identifier.
         */
        readonly id?: string;
    }
}

export type { Wallet } from '@wallet-standard/core';

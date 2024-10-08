// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import type { WalletAccount } from '@iota/wallet-standard';
import { ReadonlyWalletAccount } from '@iota/wallet-standard';

export function createMockAccount(accountOverrides: Partial<WalletAccount> = {}) {
    const keypair = new Ed25519Keypair();
    return new ReadonlyWalletAccount({
        address: keypair.getPublicKey().toIotaAddress(),
        publicKey: keypair.getPublicKey().toIotaBytes(),
        chains: ['iota:unknown'],
        features: ['iota:signAndExecuteTransactionBlock', 'iota:signTransactionBlock'],
        ...accountOverrides,
    });
}

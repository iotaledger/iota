// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import 'tsconfig-paths/register';

import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import * as bip39 from '@scure/bip39';
import { wordlist } from '@scure/bip39/wordlists/english';

export async function generateKeypairFromMnemonic(mnemonic: string) {
    return Ed25519Keypair.deriveKeypair(mnemonic);
}

export async function generateKeypair() {
    const mnemonic = bip39.generateMnemonic(wordlist, 256);
    const keypair = await generateKeypairFromMnemonic(mnemonic);
    return { mnemonic, keypair };
}

const FAUCET_HOST = 'http://127.0.0.1:9123';

export async function requestIotaFromFaucet(recipient: string) {
    await requestIotaFromFaucetV0({ host: FAUCET_HOST, recipient });
}

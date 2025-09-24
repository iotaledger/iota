// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    type BrowserPasskeyProvider,
    findCommonPublicKey,
    PasskeyKeypair,
} from '@iota/iota-sdk/keypairs/passkey';
import { useMutation } from '@tanstack/react-query';

const FIRST_MESSAGE = 'IOTA Passkey Challenge';
const SECOND_MESSAGE = 'IOTA Passkey Challenge 2';

export function useRestorePasskeyAccount() {
    return useMutation({
        mutationFn: async (provider: BrowserPasskeyProvider) => {
            const testMessage = new TextEncoder().encode(FIRST_MESSAGE);
            const possiblePks = await PasskeyKeypair.signAndRecover(provider, testMessage);

            const testMessage2 = new TextEncoder().encode(SECOND_MESSAGE);
            const possiblePks2 = await PasskeyKeypair.signAndRecover(provider, testMessage2);

            const commonPk = findCommonPublicKey(possiblePks, possiblePks2);
            const keypair = new PasskeyKeypair(commonPk.toRawBytes(), provider);

            return keypair;
        },
    });
}

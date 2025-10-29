// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    type BrowserPasskeyProvider,
    findCommonPublicKey,
    PasskeyKeypair,
} from '@iota/iota-sdk/keypairs/passkey';
import { useMutation } from '@tanstack/react-query';

export function useRestorePasskeyAccount() {
    return useMutation({
        mutationFn: async (provider: BrowserPasskeyProvider) => {
            const randomMessage1 = crypto.getRandomValues(new Uint8Array(32));
            const { credentialId, pubKeys: possiblePks } = await PasskeyKeypair.signAndRecover(
                provider,
                randomMessage1,
            );

            const randomMessage2 = crypto.getRandomValues(new Uint8Array(32));
            const { pubKeys: possiblePks2 } = await PasskeyKeypair.signAndRecover(
                provider,
                randomMessage2,
                [credentialId],
            );

            const commonPk = findCommonPublicKey(possiblePks, possiblePks2);
            const keypair = new PasskeyKeypair(commonPk.toRawBytes(), provider, credentialId);

            return keypair;
        },
    });
}

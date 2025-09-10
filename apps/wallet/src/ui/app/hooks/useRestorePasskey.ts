// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { findCommonPublicKey, PasskeyKeypair } from '@iota/iota-sdk/keypairs/passkey';
import { useMutation } from '@tanstack/react-query';
import { PASSKEY_PROVIDER } from '../components/passkey/passkey-provider';

const FIRST_MESSAGE = 'IOTA Passkey Example';
const SECOND_MESSAGE = 'IOTA Passkey Example 2';

export function useRestoreWallet() {
    return useMutation({
        mutationFn: async () => {
            const testMessage = new TextEncoder().encode(FIRST_MESSAGE);
            const possiblePks = await PasskeyKeypair.signAndRecover(PASSKEY_PROVIDER, testMessage);

            const testMessage2 = new TextEncoder().encode(SECOND_MESSAGE);
            const possiblePks2 = await PasskeyKeypair.signAndRecover(
                PASSKEY_PROVIDER,
                testMessage2,
            );

            const commonPk = findCommonPublicKey(possiblePks, possiblePks2);
            const keypair = new PasskeyKeypair(commonPk.toRawBytes(), PASSKEY_PROVIDER);

            return keypair;
        },
    });
}

// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PasskeyKeypair } from '@iota/iota-sdk/keypairs/passkey';
import { useMutation } from '@tanstack/react-query';

import { useRestoreWallet } from './useRestorePasskey';
import { PASSKEY_PROVIDER } from '../components/passkey/passkey-provider';

export function useInitializePasskey() {
    const { mutateAsync: restoreWallet } = useRestoreWallet();
    // const shouldPersistWallet = useShouldPersistWallet();
    // const setPassey = useSetPasskey();

    return useMutation({
        mutationFn: async ({ isRestore }: { isRestore: boolean }): Promise<PasskeyKeypair> => {
            try {
                const newPasskey = isRestore
                    ? await restoreWallet()
                    : await PasskeyKeypair.getPasskeyInstance(PASSKEY_PROVIDER);
                return newPasskey;
                // setPasskey(newPasskey);

                // if (shouldPersistWallet) {
                //     savePasskeyPublicKeyToLocalStorage(newPasskey);
                // }
            } catch (error) {
                const operation = isRestore ? 'restore' : 'connect';
                console.error(`Failed to ${operation} wallet:`, error);
                return await PasskeyKeypair.getPasskeyInstance(PASSKEY_PROVIDER);
            }
        },
    });
}

// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    type BrowserPasswordProviderOptions,
    PasskeyKeypair,
} from '@iota/iota-sdk/keypairs/passkey';
import { useMutation } from '@tanstack/react-query';

import { useRestoreWallet } from './useRestorePasskey';
import { createBrowserPasskeyProvider } from '../components/passkey/passkey-provider';

export function useInitializePasskey() {
    const { mutateAsync: restoreWallet } = useRestoreWallet();

    return useMutation({
        mutationFn: async ({
            isRestore,
            providerOptions,
        }: {
            isRestore: boolean;
            providerOptions?: BrowserPasswordProviderOptions;
        }): Promise<PasskeyKeypair> => {
            try {
                const provider = createBrowserPasskeyProvider({ providerOptions });
                const newPasskey = isRestore
                    ? await restoreWallet(provider)
                    : await PasskeyKeypair.getPasskeyInstance(provider);
                return newPasskey;
            } catch (error) {
                const operation = isRestore ? 'restore' : 'connect';
                throw new Error(`Failed to ${operation} passkey wallet`);
            }
        },
    });
}

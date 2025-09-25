// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PasskeyKeypair } from '@iota/iota-sdk/keypairs/passkey';
import {
    createBrowserPasswordProviderOptions,
    createBrowserPasskeyProvider,
} from '../components/passkey/passkey-provider';
import { useRestorePasskeyAccount } from './useRestorePasskeyAccount';

export function useCreatePasskeyAccount() {
    const { mutateAsync: restorePasskeyAccount } = useRestorePasskeyAccount();

    const createPasskeyAccount = async ({
        username,
        displayName,
        authenticatorAttachment,
        isRestore = false,
    }: {
        username: string;
        displayName?: string;
        authenticatorAttachment: AuthenticatorAttachment;
        isRestore?: boolean;
    }) => {
        const options = createBrowserPasswordProviderOptions({
            options: {
                authenticatorSelection: {
                    authenticatorAttachment,
                },
                user: {
                    name: username,
                    displayName: displayName || username,
                },
            },
        });

        const provider = createBrowserPasskeyProvider({ options });

        try {
            const passkeyKeypair = isRestore
                ? await restorePasskeyAccount(provider)
                : await PasskeyKeypair.getPasskeyInstance(provider);

            if (!passkeyKeypair || !passkeyKeypair.getPublicKey) {
                throw new Error('Failed to initialize passkey');
            }

            return {
                address: passkeyKeypair.getPublicKey().toIotaAddress(),
                publicKey: passkeyKeypair.getPublicKey().toBase64(),
                providerOptions: options,
            };
        } catch (error) {
            throw new Error(
                error instanceof Error
                    ? `Passkey operation failed: ${error.message}`
                    : 'Failed to create or restore passkey',
            );
        }
    };

    return { createPasskeyAccount };
}

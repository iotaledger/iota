// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PasskeyKeypair } from '@iota/iota-sdk/keypairs/passkey';
import { useRestorePasskeyAccount } from './useRestorePasskeyAccount';
import { createBrowserPasskeyProvider } from '../helpers/passkeys';

type CreatePasskeyAccountOptions =
    | {
          username: string;
          authenticatorAttachment: AuthenticatorAttachment;
          isRestore?: false | undefined;
      }
    | {
          username?: never;
          authenticatorAttachment?: never;
          isRestore: true;
      };

export function useCreatePasskeyAccount() {
    const { mutateAsync: restorePasskeyAccount } = useRestorePasskeyAccount();

    const createPasskeyAccount = async ({
        username,
        authenticatorAttachment,
        isRestore,
    }: CreatePasskeyAccountOptions) => {
        const { provider, options } = createBrowserPasskeyProvider({
            providerOptions: {
                authenticatorSelection: {
                    authenticatorAttachment,
                },
                user: {
                    name: username,
                    displayName: username,
                },
            },
        });

        try {
            const passkeyKeypair = isRestore
                ? await restorePasskeyAccount(provider)
                : await PasskeyKeypair.getPasskeyInstance(provider);

            if (!passkeyKeypair || !passkeyKeypair.getPublicKey) {
                throw new Error('Failed to initialize passkey');
            }

            const credentialId = passkeyKeypair.getCredentialId();

            if (!credentialId) {
                throw new Error('Failed to get credential ID');
            }

            return {
                address: passkeyKeypair.getPublicKey().toIotaAddress(),
                publicKey: passkeyKeypair.getPublicKey().toBase64(),
                providerOptions: options,
                credentialId: Array.from(credentialId),
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

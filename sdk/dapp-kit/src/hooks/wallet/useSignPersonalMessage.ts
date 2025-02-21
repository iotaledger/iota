// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    IotaSignPersonalMessageInput,
    IotaSignPersonalMessageOutput,
} from '@iota/wallet-standard';
import type { UseMutationOptions, UseMutationResult } from '@tanstack/react-query';
import { useMutation } from '@tanstack/react-query';

import {
    WalletFeatureNotSupportedError,
    WalletNoAccountSelectedError,
    WalletNotConnectedError,
} from '../..//errors/walletErrors.js';
import { walletMutationKeys } from '../../constants/walletMutationKeys.js';
import type { PartialBy } from '../../types/utilityTypes.js';
import { useCurrentAccount } from './useCurrentAccount.js';
import { useCurrentWallet } from './useCurrentWallet.js';

type UseSignPersonalMessageArgs = PartialBy<IotaSignPersonalMessageInput, 'account'>;

type UseSignPersonalMessageResult = IotaSignPersonalMessageOutput;

type UseSignPersonalMessageError =
    | WalletFeatureNotSupportedError
    | WalletNoAccountSelectedError
    | WalletNotConnectedError
    | Error;

type UseSignPersonalMessageMutationOptions = Omit<
    UseMutationOptions<
        UseSignPersonalMessageResult,
        UseSignPersonalMessageError,
        UseSignPersonalMessageArgs,
        unknown
    >,
    'mutationFn'
>;

/**
 * Mutation hook for prompting the user to sign a message.
 */
export function useSignPersonalMessage({
    mutationKey,
    ...mutationOptions
}: UseSignPersonalMessageMutationOptions = {}): UseMutationResult<
    UseSignPersonalMessageResult,
    UseSignPersonalMessageError,
    UseSignPersonalMessageArgs
> {
    const { currentWallet } = useCurrentWallet();
    const currentAccount = useCurrentAccount();

    return useMutation({
        mutationKey: walletMutationKeys.signPersonalMessage(mutationKey),
        mutationFn: async (signPersonalMessageArgs) => {
            if (!currentWallet) {
                throw new WalletNotConnectedError('No wallet is connected.');
            }

            const signPersonalMessageFeature = currentWallet.features['iota:signPersonalMessage'];
            if (!signPersonalMessageFeature) {
                throw new WalletFeatureNotSupportedError(
                    "This wallet doesn't support the `signPersonalMessage` feature.",
                );
            }

            const signerAccount = signPersonalMessageArgs.account ?? currentAccount;
            if (!signerAccount) {
                throw new WalletNoAccountSelectedError(
                    'No wallet account is selected to sign the personal message with.',
                );
            }

            return await signPersonalMessageFeature.signPersonalMessage({
                ...signPersonalMessageArgs,
                account: signerAccount,
            });
        },
        ...mutationOptions,
    });
}

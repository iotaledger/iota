// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import type {
	SuiDeriveAddressInput,
	SuiDeriveAddressOutput,
} from '@mysten/wallet-standard';
import type { UseMutationOptions, UseMutationResult } from '@tanstack/react-query';
import { useMutation } from '@tanstack/react-query';

import {
	WalletFeatureNotSupportedError,
	WalletNoAccountSelectedError,
	WalletNotConnectedError,
} from '../../errors/walletErrors.js';
import { walletMutationKeys } from '../../constants/walletMutationKeys.js';
import { useCurrentWallet } from './useCurrentWallet.js';

type UseDeriveAddressArgs = SuiDeriveAddressInput;

type UseDeriveAddressResult = SuiDeriveAddressOutput;

type UseDeriveAddressError =
	| WalletFeatureNotSupportedError
	| WalletNoAccountSelectedError
	| WalletNotConnectedError
	| Error;

type UseDeriveAddressMutationOptions = Omit<
	UseMutationOptions<
		UseDeriveAddressResult,
		UseDeriveAddressError,
		UseDeriveAddressArgs,
		unknown
	>,
	'mutationFn'
>;

/**
 * Mutation hook for prompting the user to sign a transaction block.
 */
export function useDeriveAddress({
	mutationKey,
	...mutationOptions
}: UseDeriveAddressMutationOptions = {}): UseMutationResult<
	UseDeriveAddressResult,
	UseDeriveAddressError,
	UseDeriveAddressArgs
> {
	const { currentWallet } = useCurrentWallet();

	return useMutation({
		mutationKey: walletMutationKeys.deriveAddress(mutationKey),
		mutationFn: async (deriveAddressArgs) => {
			if (!currentWallet) {
				throw new WalletNotConnectedError('No wallet is connected.');
			}
		
			const walletFeature = currentWallet.features['sui:deriveAddress'];
			if (!walletFeature) {
				throw new WalletFeatureNotSupportedError(
					"This wallet doesn't support the `DeriveAddress` feature.",
				);
			}

			return await walletFeature.deriveAddress({
				...deriveAddressArgs,
			});
		},
		...mutationOptions,
	});
}

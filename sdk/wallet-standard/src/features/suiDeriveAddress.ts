// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

export type SuiDeriveAddressVersion = '1.0.0';

export type SuiDeriveAddressFeature = {
	/** Namespace for the feature. */
	'sui:deriveAddress': {
		/** Version of the feature API. */
		version: SuiDeriveAddressVersion;
		deriveAddress: SuiDeriveAddressMethod;
	};
};

export type SuiDeriveAddressMethod = (input: SuiDeriveAddressInput) => Promise<SuiDeriveAddressOutput>;

export interface SuiDeriveAddressInput {
	accountIndex: number;
}

export interface SuiDeriveAddressOutput {
	address: string
}

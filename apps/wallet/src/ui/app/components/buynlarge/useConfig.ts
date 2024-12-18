// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureValue } from '@growthbook/growthbook-react';

type BuyNLargeConfig = {
	enabled: boolean;
	objectType: string;
	sheetTitle: string;
	sheetDescription: string;
	homeDescription: string;
	homeImage: string;
	backgroundColor: string;
};

export function useConfig() {
	return useFeatureValue<BuyNLargeConfig[]>('buynlargev2', []);
}

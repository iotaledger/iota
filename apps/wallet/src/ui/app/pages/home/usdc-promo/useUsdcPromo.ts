// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useFeatureIsOn, useFeatureValue } from '@growthbook/growthbook-react';

type WalletUsdcPromo = {
	promoBannerImage: string;
	promoBannerBackground: string;
	promoBannerText: string;
	promoBannerSheetTitle: string;
	promoBannerSheetContent: string;
	ctaLabel: string;
	wrappedUsdcList: string[];
};

export function useUsdcPromo() {
	const enabled = useFeatureIsOn('wallet-usdc-promo-enabled');
	const dynamicConfigs = useFeatureValue<WalletUsdcPromo>('wallet-usdc-promo', {
		promoBannerImage: '',
		promoBannerBackground: '',
		promoBannerText: '',
		promoBannerSheetTitle: '',
		promoBannerSheetContent: '',
		ctaLabel: '',
		wrappedUsdcList: [],
	});

	return {
		...dynamicConfigs,
		enabled,
	};
}

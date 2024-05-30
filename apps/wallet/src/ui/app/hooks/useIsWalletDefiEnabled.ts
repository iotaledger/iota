// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import useAppSelector from '_app/hooks/useAppSelector';
import { FEATURES } from '_shared/experimentation/features';
import { useFeature } from '@growthbook/growthbook-react';
import { Network } from '@mysten/iota.js/client';

export function useIsWalletDefiEnabled() {
	const isDefiWalletEnabled = useFeature<boolean>(FEATURES.WALLET_DEFI).value;
	const { network } = useAppSelector((state) => state.app);

	return network === Network.Mainnet && isDefiWalletEnabled;
}

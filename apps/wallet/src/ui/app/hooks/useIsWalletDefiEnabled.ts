// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import useAppSelector from '_app/hooks/useAppSelector';
import { Feature } from '_shared/experimentation/features';
import { useFeature } from '@growthbook/growthbook-react';
import { Network } from '@iota/iota-sdk/client';

export function useIsWalletDefiEnabled() {
    const isDefiWalletEnabled = useFeature<boolean>(Feature.WalletDefi).value;
    const { network } = useAppSelector((state) => state.app);

    return network === Network.Mainnet && isDefiWalletEnabled;
}

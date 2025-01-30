// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import { useBalanceInUSD } from './useTokenPrice';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useBalance } from './useBalance';
import { formatBalanceToUSD } from '../utils';
import { Network } from '@iota/iota-sdk/client';

export function useGetFiatBalance(network: Network): string | null {
    const account = useCurrentAccount();
    const address = account?.address ?? '';
    const { data: coinBalance } = useBalance(address);
    const balance = useBalanceInUSD(IOTA_TYPE_ARG, coinBalance?.totalBalance ?? 0, network);

    if (!balance) return null;

    return formatBalanceToUSD(balance);
}

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import { useBalance } from './useBalance';
import { useTokenPrice } from './useTokenPrice';
import { CoinFormat, formatBalance, useCoinMetadata } from './useFormatCoin';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export function useTotalFiatBalance() {
    const { data: { price } = {} } = useTokenPrice('iota');
    const address = useCurrentAccount()?.address;
    const { data: coinBalance } = useBalance(address!);
    const totalBalance = Number(coinBalance?.totalBalance);
    const queryResult = useCoinMetadata(IOTA_TYPE_ARG);
    const iotaToFiat = totalBalance && price ? Number(totalBalance) * Number(price) : 0;
    const formatted = formatBalance(iotaToFiat, queryResult.data?.decimals ?? 0, CoinFormat.FULL);
    return price ? `${coinToFiat(formatted, price)}` : null;
}

function coinToFiat(coinBalance: string, coinPrice: string): string {
    const totalBalanceInUsd = Number(coinBalance) * Number(coinPrice);
    return Number(totalBalanceInUsd).toLocaleString('en', {
        style: 'currency',
        currency: 'USD',
    });
}

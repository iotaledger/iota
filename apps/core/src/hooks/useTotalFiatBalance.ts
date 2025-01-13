// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import { useBalance } from './useBalance';
import { useTokenPrice } from './useTokenPrice';
import { CoinFormat, formatBalance, useCoinMetadata } from './useFormatCoin';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { FiatTokenName } from '../enums';

export function useTotalFiatBalance(): string {
    const { data: { price } = {} } = useTokenPrice(FiatTokenName.Iota);
    const address = useCurrentAccount()?.address;

    if (!address) {
        return '';
    }

    const { data: coinBalance } = useBalance(address);
    const totalBalance = Number(coinBalance?.totalBalance || 0);
    const queryResult = useCoinMetadata(IOTA_TYPE_ARG);
    const iotaToFiat = totalBalance && price ? totalBalance * Number(price || 0) : 0;
    const formatted = formatBalance(iotaToFiat, queryResult.data?.decimals || 0, CoinFormat.FULL);
    return price ? `${coinToFiat(formatted, price)}` : '';
}

function coinToFiat(coinBalance: string, coinPrice: string): string {
    const totalBalanceInUsd = Number(coinBalance) * Number(coinPrice);
    return Number(totalBalanceInUsd).toLocaleString('en', {
        style: 'currency',
        currency: 'USD',
    });
}

// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinStruct } from '@iota/iota-sdk/client';

export function sumCoinBalances(coins: CoinStruct[]): bigint {
    return coins.reduce((acc, coin) => acc + getBalanceFromCoinStruct(coin), BigInt(0));
}

function getBalanceFromCoinStruct(coin: CoinStruct): bigint {
    return BigInt(coin.balance);
}

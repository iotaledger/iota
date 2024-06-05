// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { parseAmount } from '_src/ui/app/helpers';
import { type CoinStruct } from '@iota/iota.js/client';
import { TransactionBlock } from '@iota/iota.js/transactions';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';

interface Options {
    coinType: string;
    to: string;
    amount: string;
    coinDecimals: number;
    isPayAllIota: boolean;
    coins: CoinStruct[];
}

export function createTokenTransferTransaction({
    to,
    amount,
    coins,
    coinType,
    coinDecimals,
    isPayAllIota,
}: Options) {
    const tx = new TransactionBlock();

    if (isPayAllIota && coinType === IOTA_TYPE_ARG) {
        tx.transferObjects([tx.gas], to);
        tx.setGasPayment(
            coins
                .filter((coin) => coin.coinType === coinType)
                .map((coin) => ({
                    objectId: coin.coinObjectId,
                    digest: coin.digest,
                    version: coin.version,
                })),
        );

        return tx;
    }

    const bigIntAmount = parseAmount(amount, coinDecimals);
    const [primaryCoin, ...mergeCoins] = coins.filter((coin) => coin.coinType === coinType);

    if (coinType === IOTA_TYPE_ARG) {
        const coin = tx.splitCoins(tx.gas, [bigIntAmount]);
        tx.transferObjects([coin], to);
    } else {
        const primaryCoinInput = tx.object(primaryCoin.coinObjectId);
        if (mergeCoins.length) {
            // TODO: This could just merge a subset of coins that meet the balance requirements instead of all of them.
            tx.mergeCoins(
                primaryCoinInput,
                mergeCoins.map((coin) => tx.object(coin.coinObjectId)),
            );
        }
        const coin = tx.splitCoins(primaryCoinInput, [bigIntAmount]);
        tx.transferObjects([coin], to);
    }

    return tx;
}

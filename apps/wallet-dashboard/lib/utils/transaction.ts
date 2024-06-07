// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinStruct } from '@iota/iota.js/client';
import { TransactionBlock } from '@iota/iota.js/transactions';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import { parseAmount } from '../helpers/parseAmount';

interface CreateTokenTransferTransactionOptions {
    coinType: string;
    recipientAddress: string;
    amount: string;
    coinDecimals: number;
    coins: CoinStruct[];
}

export function createTokenTransferTransaction({
    recipientAddress,
    amount,
    coins,
    coinType,
    coinDecimals,
}: CreateTokenTransferTransactionOptions) {
    const tx = new TransactionBlock();

    // https://github.com/iotaledger/iota/issues/598
    // Optimization: Avoid splitting coins when sending full balances

    const bigIntAmount = parseAmount(amount, coinDecimals);
    const [primaryCoin, ...mergeCoins] = coins.filter((coin) => coin.coinType === coinType);

    if (coinType === IOTA_TYPE_ARG) {
        const coin = tx.splitCoins(tx.gas, [bigIntAmount]);
        tx.transferObjects([coin], recipientAddress);
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
        tx.transferObjects([coin], recipientAddress);
    }

    return tx;
}

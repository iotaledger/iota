// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinStruct } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    AccountsContractMethod,
    ChainData,
    CoreContract,
    getHname,
    IscTransaction,
    L2_FROM_L1_GAS_BUDGET,
} from '@iota/isc-sdk';

interface Options {
    amount: bigint;
    receivingAddress: string;
    coinType: string;
    chain: ChainData;
    coins?: CoinStruct[];
}

export function createDepositTransactionL1({
    amount,
    receivingAddress,
    coins = [],
    coinType = IOTA_TYPE_ARG,
    chain,
}: Options) {
    const iscTx = new IscTransaction(chain);
    const bag = iscTx.newBag();

    const isIotaCoinType = coinType === IOTA_TYPE_ARG;
    // If the coin type is IOTA, we need to add the L2 gas budget to the amount otherwise for native coins we only need the gas budget
    const amountToPlace = isIotaCoinType ? amount + L2_FROM_L1_GAS_BUDGET : L2_FROM_L1_GAS_BUDGET;

    // add iota coins to the bag
    const coin = iscTx.coinFromAmount({ amount: amountToPlace });
    iscTx.placeCoinInBag({ coin, bag });

    // If the coin type is not IOTA, we need to add the coins to the bag
    if (!isIotaCoinType) {
        const totalCoinBalance = coins.reduce((acc, { balance }) => {
            return BigInt(acc) + BigInt(balance);
        }, BigInt(0));
        const isTransferAllObjects = totalCoinBalance === amount;

        const tx = iscTx.transaction();

        // merge coins of the selected type
        const [primaryCoin, ...mergeCoins] = coins.filter((coin) => coin.coinType === coinType);
        const primaryCoinInput = tx.object(primaryCoin.coinObjectId);

        if (mergeCoins.length) {
            tx.mergeCoins(
                primaryCoinInput,
                mergeCoins.map((coin) => tx.object(coin.coinObjectId)),
            );
        }
        const coin = isTransferAllObjects
            ? primaryCoinInput
            : tx.splitCoins(primaryCoinInput, [amount]);

        iscTx.placeCoinInBag({
            bag,
            coin,
            coinType,
        });
    }

    iscTx.createAndSendToEvm({
        bag,
        transfers: [[coinType, amount]],
        address: receivingAddress,
        accountsContract: getHname(CoreContract.Accounts),
        accountsFunction: getHname(AccountsContractMethod.TransferAllowanceTo),
    });
    const transaction = iscTx.build();

    return transaction;
}

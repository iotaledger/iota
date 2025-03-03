// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type CoinStruct } from '@iota/iota-sdk/client';
import { Transaction, TransactionObjectArgument } from '@iota/iota-sdk/transactions';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { parseAmount } from '../parseAmount';

interface Options {
    coinType: string;
    to: string;
    amount: string;
    coinDecimals: number;
    coins: CoinStruct[];
}

export function createTokenTransferTransaction({
    to,
    amount,
    coins,
    coinType,
    coinDecimals,
}: Options) {
    const tx = new Transaction();

    const totalBalance = coins.reduce((acc, { balance }) => {
        return BigInt(acc) + BigInt(balance);
    }, BigInt(0));

    const bigIntAmount = parseAmount(amount, coinDecimals);
    const isTransferAllObjects = totalBalance === bigIntAmount;

    if (coinType === IOTA_TYPE_ARG) {
        sendCoins(tx, to, tx.gas, isTransferAllObjects, bigIntAmount);
    } else {
        handleCoinTransfer(tx, to, coins, coinType, isTransferAllObjects, bigIntAmount);
    }

    return tx;
}

function handleCoinTransfer(
    tx: Transaction,
    to: string,
    coins: CoinStruct[],
    coinType: string,
    isTransferAllObjects: boolean,
    bigIntAmount: bigint,
) {
    const [primaryCoin, ...mergeCoins] = coins.filter((coin) => coin.coinType === coinType);
    const primaryCoinInput = tx.object(primaryCoin.coinObjectId);

    if (mergeCoins.length) {
        // TODO: This could just merge a subset of coins that meet the balance requirements instead of all of them.
        tx.mergeCoins(
            primaryCoinInput,
            mergeCoins.map((coin) => tx.object(coin.coinObjectId)),
        );
    }
    sendCoins(tx, to, primaryCoinInput, isTransferAllObjects, bigIntAmount);
}

function sendCoins(
    tx: Transaction,
    to: string,
    coinObject: TransactionObjectArgument | string,
    isTransferAllObjects: boolean,
    bigIntAmount: bigint,
) {
    if (isTransferAllObjects) {
        tx.transferObjects([coinObject], to);
    } else {
        const coin = tx.splitCoins(coinObject, [bigIntAmount]);
        tx.transferObjects([coin], to);
    }
}

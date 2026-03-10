// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_CLOCK_OBJECT_ID } from '@iota/iota-sdk/utils';

export function buildCreateAuctionTransaction(
    auctionPackageId: string,
    iotaNames: string,
    auctionHouseId: string,
    senderAddress: string,
    bidAmountNanos: bigint,
    targetName: string,
) {
    const transaction = new Transaction();

    transaction.setSender(senderAddress);
    const coin = transaction.splitCoins(transaction.gas, [bidAmountNanos]);

    transaction.moveCall({
        target: `${auctionPackageId}::auction::start_auction_and_place_bid`,
        arguments: [
            transaction.object(auctionHouseId),
            transaction.object(iotaNames),
            transaction.pure.string(targetName),
            coin,
            transaction.object(IOTA_CLOCK_OBJECT_ID),
        ],
    });

    return transaction;
}

export function buildPlaceBidTransaction(
    auctionPackageId: string,
    auctionHouseId: string,
    senderAddress: string,
    bidAmountNanos: bigint,
    targetName: string,
) {
    const transaction = new Transaction();

    transaction.setSender(senderAddress);
    const coin = transaction.splitCoins(transaction.gas, [bidAmountNanos]);

    transaction.moveCall({
        target: `${auctionPackageId}::auction::place_bid`,
        arguments: [
            transaction.object(auctionHouseId),
            transaction.pure.string(targetName),
            coin,
            transaction.object(IOTA_CLOCK_OBJECT_ID),
        ],
    });

    return transaction;
}

export function buildClaimNameTransaction(
    auctionPackageId: string,
    auctionHouseId: string,
    senderAddress: string,
    targetName: string,
) {
    const transaction = new Transaction();

    transaction.setSender(senderAddress);

    const [nft] = transaction.moveCall({
        target: `${auctionPackageId}::auction::claim`,
        arguments: [
            transaction.object(auctionHouseId),
            transaction.pure.string(targetName),
            transaction.object(IOTA_CLOCK_OBJECT_ID),
        ],
    });

    transaction.transferObjects([nft], senderAddress);

    return transaction;
}

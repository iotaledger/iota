// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaClient, IotaObjectData } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';
import { STARDUST_PACKAGE_ID } from '../../constants/migration.constants';
import { IOTA_COIN_TYPE } from '../../constants/coins.constants';

type NestedResultType = {
    $kind: 'NestedResult';
    NestedResult: [number, number];
};

type ExpirationUnlockCondition = {
    owner: string;
    return_address: string;
    unix_time: number;
};
type StorageDepositReturnUnlockCondition = {
    return_address: string;
    return_amount: string;
};
type TimelockUnlockCondition = {
    unix_time: number;
};

export type CommonOutputObject = {
    id: { id: string };
    balance: string;
    native_tokens: {
        type: string;
        fields: { id: { id: string }; size: string };
    };
};

export interface CommonOutputObjectWithUc extends CommonOutputObject {
    expiration_uc?: {
        type: string;
        fields: ExpirationUnlockCondition;
    };
    storage_deposit_return_uc?: {
        type: string;
        fields: StorageDepositReturnUnlockCondition;
    };
    timelock_uc?: {
        type: string;
        fields: TimelockUnlockCondition;
    };
}

export interface BasicOutputObject extends CommonOutputObjectWithUc {
    metadata?: number[];
    tag?: number[];
    sender?: string;
}

export interface NftOutputObject extends CommonOutputObjectWithUc {}

export async function getNativeTokenTypesFromBag(
    bagId: string,
    client: IotaClient,
): Promise<string[]> {
    const nativeTokenDynamicFields = await client.getDynamicFields({
        parentId: bagId,
    });
    const nativeTokenTypes: string[] = [];
    for (const nativeToken of nativeTokenDynamicFields.data) {
        nativeTokenTypes.push(nativeToken?.name?.value as string);
    }

    return nativeTokenTypes;
}

export async function createMigrationTransaction(
    client: IotaClient,
    address: string,
    basicOutputs: IotaObjectData[] = [],
    nftOutputs: IotaObjectData[] = [],
): Promise<Transaction> {
    const ptb = new Transaction();

    const coinsFromBasicOutputs: NestedResultType[] = [];

    // Basics
    for (const basicOutputObject of basicOutputs) {
        const basicOutputObjectId = basicOutputObject.objectId;

        const bagId = (basicOutputObject.content as unknown as { fields: BasicOutputObject }).fields
            .native_tokens.fields.id.id;
        const bagSize = (basicOutputObject.content as unknown as { fields: BasicOutputObject })
            .fields.native_tokens.fields.size;

        const nativeTokenTypes: string[] =
            Number(bagSize) > 0 ? await getNativeTokenTypesFromBag(bagId, client) : [];
        // console.log('Native Token Types:', bagId, Number(bagSize), nativeTokenTypes);
        const migratableResult = ptb.moveCall({
            target: `${STARDUST_PACKAGE_ID}::basic_output::extract_assets`,
            typeArguments: [IOTA_COIN_TYPE],
            arguments: [ptb.object(basicOutputObjectId)],
        });

        // eslint-disable-next-line prefer-const
        let [balance, nativeTokensBag] = migratableResult;
        // let nativeTokensBag = initialNativeTokensBag;

        // Convert Balance in Coin
        const [coin] = ptb.moveCall({
            target: '0x02::coin::from_balance',
            typeArguments: [IOTA_COIN_TYPE],
            arguments: [ptb.object(balance)],
        });

        coinsFromBasicOutputs.push(coin);

        for (const nativeTokenType of nativeTokenTypes) {
            console.log('Native Token Type:', nativeTokenType, JSON.stringify(basicOutputObject, null, 2));
            // Convert NativeTokenBag in Native token and sent to address
            [nativeTokensBag] = ptb.moveCall({
                target: '0x107a::utilities::extract_and_send_to',
                typeArguments: [nativeTokenType],
                arguments: [ptb.object(nativeTokensBag), ptb.pure.address(address)],
            });
        }

        ptb.moveCall({
            target: '0x02::bag::destroy_empty', // Destroy empty native tokens
            arguments: [ptb.object(nativeTokensBag)],
        });
    }

    const coinsFromNftOutputs: NestedResultType[] = [];
    const nftsFromNftOutputs: NestedResultType[] = [];

    // NFTs
    for (const nftOutputObject of nftOutputs) {
        const nftOutputObjectId = nftOutputObject.objectId;

        const bagId = (nftOutputObject.content as unknown as { fields: NftOutputObject }).fields
            .native_tokens.fields.id.id;
        const bagSize = (nftOutputObject.content as unknown as { fields: BasicOutputObject }).fields
            .native_tokens.fields.size;

        const nativeTokenTypes: string[] =
            Number(bagSize) > 0 ? await getNativeTokenTypesFromBag(bagId, client) : [];
        // console.log('Native Token Types NFT:', bagId, Number(bagSize), nativeTokenTypes);
        const migratableResult = ptb.moveCall({
            target: `${STARDUST_PACKAGE_ID}::nft_output::extract_assets`,
            typeArguments: [IOTA_COIN_TYPE],
            arguments: [ptb.object(nftOutputObjectId)],
        });

        // eslint-disable-next-line prefer-const
        let [balance, nativeTokensBag, nft] = migratableResult;
        // let nativeTokensBag = initialNativeTokensBag;
        nftsFromNftOutputs.push(nft);

        // Convert Balance in Coin
        const [coin] = ptb.moveCall({
            target: '0x02::coin::from_balance',
            typeArguments: [IOTA_COIN_TYPE],
            arguments: [ptb.object(balance)],
        });
        coinsFromNftOutputs.push(coin);

        for (const nativeTokenType of nativeTokenTypes) {
            // Convert NativeTokenBag in Native token and sent to address
            [nativeTokensBag] = ptb.moveCall({
                target: '0x107a::utilities::extract_and_send_to',
                typeArguments: [nativeTokenType],
                arguments: [ptb.object(nativeTokensBag), ptb.pure.address(address)],
            });
        }

        ptb.moveCall({
            target: '0x02::bag::destroy_empty', // Destroy empty native tokens
            arguments: [ptb.object(nativeTokensBag)],
        });
    }

    const coinOne = coinsFromBasicOutputs.shift() || coinsFromNftOutputs.shift();
    const remainingCoins = [...coinsFromBasicOutputs, ...coinsFromNftOutputs];

    if (coinOne) {
        if (remainingCoins.length > 0) {
            ptb.mergeCoins(coinOne, remainingCoins);
        }
        ptb.transferObjects([coinOne, ...nftsFromNftOutputs], ptb.pure.address(address));
    } else {
        ptb.transferObjects([...nftsFromNftOutputs], ptb.pure.address(address));
    }
    console.log('Migration Transaction:', ptb);
    return ptb;
}

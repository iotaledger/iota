// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DynamicFieldInfo, IotaClient, IotaObjectData } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';
import { STARDUST_PACKAGE_ID } from '../../constants/migration.constants';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    BasicOutputObject,
    BasicOutputObjectSchema,
    NftOutputObject,
    NftOutputObjectSchema,
} from './types';

type NestedResultType = {
    $kind: 'NestedResult';
    NestedResult: [number, number];
};

export async function getNativeTokensFromBag(bagId: string, client: IotaClient) {
    const nativeTokenDynamicFields = await client.getDynamicFields({
        parentId: bagId,
    });
    const nativeTokenTypes: DynamicFieldInfo[] = [];
    for (const nativeToken of nativeTokenDynamicFields.data) {
        nativeTokenTypes.push(nativeToken);
    }

    return nativeTokenTypes;
}

export async function getNativeTokenTypesFromBag(
    bagId: string,
    client: IotaClient,
): Promise<string[]> {
    const nativeTokenDynamicFields = await client.getDynamicFields({
        parentId: bagId,
    });
    return nativeTokenDynamicFields.data.map(({ name }) => name.value as string);
}

export function validateBasicOutputObject(outputObject: IotaObjectData): BasicOutputObject {
    if (outputObject.content?.dataType !== 'moveObject') {
        throw new Error('Invalid basic output object');
    }
    const result = BasicOutputObjectSchema.safeParse(outputObject.content.fields);
    if (!result.success) {
        throw new Error('Invalid basic output object content');
    }
    return result.data;
}

export function validateNftOutputObject(outputObject: IotaObjectData): NftOutputObject {
    if (outputObject.content?.dataType !== 'moveObject') {
        throw new Error('Invalid nft output object');
    }
    const result = NftOutputObjectSchema.safeParse(outputObject.content.fields);
    if (!result.success) {
        throw new Error('Invalid nft output object content');
    }
    return result.data;
}

export async function createMigrationTransaction(
    client: IotaClient,
    address: string,
    basicOutputs: IotaObjectData[],
    nftOutputs: IotaObjectData[],
): Promise<Transaction> {
    const ptb = new Transaction();

    const coinsFromBasicOutputs: NestedResultType[] = [];

    // Basic Outputs
    for (const basicOutputObject of basicOutputs) {
        const validatedOutputObject = validateBasicOutputObject(basicOutputObject);
        const basicOutputObjectId = validatedOutputObject.id.id;
        const bagId = validatedOutputObject.native_tokens.fields.id.id;
        const bagSize = validatedOutputObject.native_tokens.fields.size;
        const nativeTokenTypes: string[] =
            Number(bagSize) > 0 ? await getNativeTokenTypesFromBag(bagId, client) : [];

        const migratableResult = ptb.moveCall({
            target: `${STARDUST_PACKAGE_ID}::basic_output::extract_assets`,
            typeArguments: [IOTA_TYPE_ARG],
            arguments: [ptb.object(basicOutputObjectId)],
        });

        const balance = migratableResult[0];
        let nativeTokensBag = migratableResult[1];

        // Convert Balance in Coin
        const [coin] = ptb.moveCall({
            target: '0x02::coin::from_balance',
            typeArguments: [IOTA_TYPE_ARG],
            arguments: [ptb.object(balance)],
        });

        coinsFromBasicOutputs.push(coin);

        for (const nativeTokenType of nativeTokenTypes) {
            [nativeTokensBag] = ptb.moveCall({
                target: '0x107a::utilities::extract_and_send_to',
                typeArguments: [nativeTokenType],
                arguments: [ptb.object(nativeTokensBag), ptb.pure.address(address)],
            });
        }

        ptb.moveCall({
            target: '0x02::bag::destroy_empty',
            arguments: [ptb.object(nativeTokensBag)],
        });
    }

    // NFT Outputs
    const coinsFromNftOutputs: NestedResultType[] = [];
    const nftsFromNftOutputs: NestedResultType[] = [];

    for (const nftOutputObject of nftOutputs) {
        const validatedOutputObject = validateNftOutputObject(nftOutputObject);
        const nftOutputObjectId = validatedOutputObject.id.id;
        const bagId = validatedOutputObject.native_tokens.fields.id.id;
        const bagSize = validatedOutputObject.native_tokens.fields.size;
        const nativeTokenTypes: string[] =
            Number(bagSize) > 0 ? await getNativeTokenTypesFromBag(bagId, client) : [];

        const migratableResult = ptb.moveCall({
            target: `${STARDUST_PACKAGE_ID}::nft_output::extract_assets`,
            typeArguments: [IOTA_TYPE_ARG],
            arguments: [ptb.object(nftOutputObjectId)],
        });

        const balance = migratableResult[0];
        let nativeTokensBag = migratableResult[1];
        const nft = migratableResult[2];

        nftsFromNftOutputs.push(nft);

        // Convert Balance in Coin
        const [coin] = ptb.moveCall({
            target: '0x02::coin::from_balance',
            typeArguments: [IOTA_TYPE_ARG],
            arguments: [ptb.object(balance)],
        });
        coinsFromNftOutputs.push(coin);

        for (const nativeTokenType of nativeTokenTypes) {
            [nativeTokensBag] = ptb.moveCall({
                target: '0x107a::utilities::extract_and_send_to',
                typeArguments: [nativeTokenType],
                arguments: [ptb.object(nativeTokensBag), ptb.pure.address(address)],
            });
        }

        ptb.moveCall({
            target: '0x02::bag::destroy_empty',
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
    }

    return ptb;
}

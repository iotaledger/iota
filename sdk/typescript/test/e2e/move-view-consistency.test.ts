// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { beforeAll, describe, expect, it } from 'vitest';

import type {
    CoinStruct,
    IotaMoveViewCallResults,
    MoveValue,
} from '../../src/client/types/generated';
import { IotaGraphQLClient } from '../../src/graphql';
import { graphql } from '../../src/graphql/schemas/2025.2';
import { IOTA_TYPE_ARG } from '../../src/utils';
import { setup, TestToolbox } from './utils/setup';

function moveValueToBigInt(value: MoveValue): bigint {
    if (typeof value === 'string' || typeof value === 'number') {
        return BigInt(value);
    }

    throw new Error(`Unsupported MoveValue for u64 conversion: ${JSON.stringify(value)}`);
}

function isViewSuccess(
    result: IotaMoveViewCallResults,
): result is Extract<IotaMoveViewCallResults, { functionReturnValues: MoveValue[] }> {
    return 'functionReturnValues' in result;
}

async function getAllCoinsByType(toolbox: TestToolbox, coinType: string): Promise<CoinStruct[]> {
    const coins: CoinStruct[] = [];
    let cursor: string | null | undefined = null;
    let hasNextPage = true;

    while (hasNextPage) {
        const page = await toolbox.client.getCoins({
            owner: toolbox.address(),
            coinType,
            cursor,
        });

        coins.push(...page.data);
        cursor = page.nextCursor;
        hasNextPage = page.hasNextPage;
    }

    return coins;
}

describe('Move view consistency across transports', () => {
    const graphQLClient = new IotaGraphQLClient({
        url: 'http://127.0.0.1:9125',
    });

    let toolbox: TestToolbox;
    let coinId: string;
    let coinBalance: bigint;
    let totalBalance: bigint;

    beforeAll(async () => {
        toolbox = await setup();

        const coins = await toolbox.client.getCoins({
            owner: toolbox.address(),
            coinType: IOTA_TYPE_ARG,
        });

        expect(coins.data.length).toBeGreaterThan(0);

        coinId = coins.data[0].coinObjectId;
        coinBalance = BigInt(coins.data[0].balance);

        totalBalance = BigInt(
            (
                await toolbox.client.getBalance({
                    owner: toolbox.address(),
                    coinType: IOTA_TYPE_ARG,
                })
            ).totalBalance,
        );
    });

    it('calls a non-annotated public function via JSON-RPC view', async () => {
        const result = await toolbox.client.view({
            functionName: '0x2::coin::value',
            typeArgs: [IOTA_TYPE_ARG],
            arguments: [coinId],
        });

        expect(isViewSuccess(result)).toBe(true);
        if (!isViewSuccess(result)) {
            throw new Error(`View call failed: ${result.executionError}`);
        }

        const value = moveValueToBigInt(result.functionReturnValues[0]);
        expect(value).toEqual(coinBalance);
    });

    it('returns the same value via GraphQL moveViewCall', async () => {
        const response = await graphQLClient.query({
            query: graphql(`
                query MoveViewCoinValue(
                    $functionName: String!
                    $typeArgs: [String!]
                    $arguments: [JSON!]
                ) {
                    moveViewCall(
                        functionName: $functionName
                        typeArgs: $typeArgs
                        arguments: $arguments
                    ) {
                        error
                        results
                    }
                }
            `),
            variables: {
                functionName: '0x2::coin::value',
                typeArgs: [IOTA_TYPE_ARG],
                arguments: [coinId],
            },
        });

        expect(response.data?.moveViewCall.error).toBeNull();
        expect(response.data?.moveViewCall.results?.length).toEqual(1);

        const result0 = response.data!.moveViewCall.results![0] as unknown;
        const value = BigInt(typeof result0 === 'number' ? result0 : String(result0));
        expect(value).toEqual(coinBalance);
    });

    it('matches node getBalance totals with sum of view results', async () => {
        const coins = await getAllCoinsByType(toolbox, IOTA_TYPE_ARG);

        const viewValues = await Promise.all(
            coins.map(async (coin) => {
                const result = await toolbox.client.view({
                    functionName: '0x2::coin::value',
                    typeArgs: [IOTA_TYPE_ARG],
                    arguments: [coin.coinObjectId],
                });

                if ('executionError' in result) {
                    throw new Error(`View call failed: ${result.executionError}`);
                }

                return moveValueToBigInt(result.functionReturnValues[0]);
            }),
        );

        const sum = viewValues.reduce((acc, value) => acc + value, 0n);
        expect(sum).toEqual(totalBalance);
    });
});

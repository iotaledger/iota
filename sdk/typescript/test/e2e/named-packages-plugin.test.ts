// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';

import { IotaGraphQLClient } from '../../src/graphql/client';
import { namedPackagesPlugin, Transaction } from '../../src/transactions';
import { normalizeIotaAddress } from '../../src/utils';

Transaction.registerGlobalSerializationPlugin(
    'namedPackagesPlugin',
    namedPackagesPlugin({
        iotaGraphQLClient: new IotaGraphQLClient({
            url: 'http://127.0.0.1:9125',
        }),
        overrides: {
            packages: {
                '@framework/std': '0x1',
                '@framework/std/1': '0x1',
            },
            types: {
                '@framework/std::string::String': '0x1::string::String',
                '@framework/std::vector::empty<@framework/std::string::String>':
                    '0x1::vector::empty<0x1::string::String>',
            },
        },
    }),
);

describe('Name Resolution Plugin (.move)', () => {
    it('Should replace names in a given PTB', async () => {
        const transaction = new Transaction();

        // replace .move names properly
        transaction.moveCall({
            target: '@framework/std::string::utf8',
            arguments: [transaction.pure.string('Hello, world!')],
        });

        // replace type args properly
        transaction.moveCall({
            target: '@framework/std::vector::empty',
            typeArguments: ['@framework/std::string::String'],
        });

        // replace nested type args properly
        transaction.moveCall({
            target: '@framework/std/1::vector::empty',
            typeArguments: ['@framework/std::vector::empty<@framework/std::string::String>'],
        });

        // replace type args in `MakeMoveVec` call.
        transaction.makeMoveVec({
            type: '@framework/std::string::String',
            elements: [transaction.pure.string('Hello, world!')],
        });

        const json = JSON.parse(await transaction.toJSON());

        expect(json.commands[0].MoveCall.package).toBe(normalizeIotaAddress('0x1'));
        expect(json.commands[1].MoveCall.typeArguments[0]).toBe(`0x1::string::String`);
        expect(json.commands[2].MoveCall.package).toBe(normalizeIotaAddress('0x1'));
        expect(json.commands[2].MoveCall.typeArguments[0]).toBe(
            `0x1::vector::empty<0x1::string::String>`,
        );
    });

    // TODO: Add some tests utilizing live GraphQL Queries (mainnet / testnet),
    // not just overrides.
});

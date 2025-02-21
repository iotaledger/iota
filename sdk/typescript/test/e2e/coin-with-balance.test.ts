// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { resolve } from 'path';
import { fromHEX, toB64 } from '@iota/bcs';
import { beforeAll, describe, expect, it } from 'vitest';

import { bcs } from '../../src/bcs';
import { Ed25519Keypair } from '../../src/keypairs/ed25519';
import { Transaction } from '../../src/transactions';
import { coinWithBalance } from '../../src/transactions/intents/CoinWithBalance';
import { normalizeIotaAddress } from '../../src/utils';
import { setup, TestToolbox } from './utils/setup';

describe('coinWithBalance', () => {
    let toolbox: TestToolbox;
    let publishToolbox: TestToolbox;
    let packageId: string;
    let testType: string;

    beforeAll(async () => {
        [toolbox, publishToolbox] = await Promise.all([setup(), setup()]);
        const packagePath = resolve(__dirname, './data/coin_metadata');
        packageId = await publishToolbox.getPackage(packagePath);
        testType = normalizeIotaAddress(packageId) + '::test::TEST';
    });

    it('works with iota', async () => {
        const tx = new Transaction();
        const receiver = new Ed25519Keypair();

        tx.transferObjects(
            [
                coinWithBalance({
                    type: 'gas',
                    balance: 12345n,
                }),
            ],
            receiver.toIotaAddress(),
        );
        tx.setSender(publishToolbox.keypair.toIotaAddress());

        expect(
            JSON.parse(
                await tx.toJSON({
                    supportedIntents: ['CoinWithBalance'],
                }),
            ),
        ).toEqual({
            expiration: null,
            gasData: {
                budget: null,
                owner: null,
                payment: null,
                price: null,
            },
            inputs: [
                {
                    Pure: {
                        bytes: toB64(fromHEX(receiver.toIotaAddress())),
                    },
                },
            ],
            sender: publishToolbox.keypair.toIotaAddress(),
            commands: [
                {
                    $Intent: {
                        data: {
                            balance: '12345',
                            type: 'gas',
                        },
                        inputs: {},
                        name: 'CoinWithBalance',
                    },
                },
                {
                    TransferObjects: {
                        objects: [
                            {
                                Result: 0,
                            },
                        ],
                        address: {
                            Input: 0,
                        },
                    },
                },
            ],
            version: 2,
        });

        expect(
            JSON.parse(
                await tx.toJSON({
                    supportedIntents: [],
                    client: toolbox.client,
                }),
            ),
        ).toEqual({
            expiration: null,
            gasData: {
                budget: null,
                owner: null,
                payment: null,
                price: null,
            },
            inputs: [
                {
                    Pure: {
                        bytes: toB64(fromHEX(receiver.toIotaAddress())),
                    },
                },
                {
                    Pure: {
                        bytes: toB64(bcs.u64().serialize(12345).toBytes()),
                    },
                },
            ],
            sender: publishToolbox.keypair.toIotaAddress(),
            commands: [
                {
                    SplitCoins: {
                        coin: {
                            GasCoin: true,
                        },
                        amounts: [
                            {
                                Input: 1,
                            },
                        ],
                    },
                },
                {
                    TransferObjects: {
                        objects: [
                            {
                                NestedResult: [0, 0],
                            },
                        ],
                        address: {
                            Input: 0,
                        },
                    },
                },
            ],
            version: 2,
        });

        const { digest } = await toolbox.client.signAndExecuteTransaction({
            transaction: tx,
            signer: publishToolbox.keypair,
        });

        const result = await toolbox.client.waitForTransaction({
            digest,
            options: { showEffects: true, showBalanceChanges: true },
        });

        expect(result.effects?.status.status).toBe('success');
        expect(
            result.balanceChanges?.find(
                (change) =>
                    typeof change.owner === 'object' &&
                    'AddressOwner' in change.owner &&
                    change.owner.AddressOwner === receiver.toIotaAddress(),
            ),
        ).toEqual({
            amount: '12345',
            coinType: '0x2::iota::IOTA',
            owner: {
                AddressOwner: receiver.toIotaAddress(),
            },
        });
    });

    it('works with custom coin', async () => {
        const tx = new Transaction();
        const receiver = new Ed25519Keypair();

        tx.transferObjects(
            [
                coinWithBalance({
                    type: testType,
                    balance: 1n,
                }),
            ],
            receiver.toIotaAddress(),
        );
        tx.setSender(publishToolbox.keypair.toIotaAddress());

        expect(
            JSON.parse(
                await tx.toJSON({
                    supportedIntents: ['CoinWithBalance'],
                }),
            ),
        ).toEqual({
            expiration: null,
            gasData: {
                budget: null,
                owner: null,
                payment: null,
                price: null,
            },
            inputs: [
                {
                    Pure: {
                        bytes: toB64(fromHEX(receiver.toIotaAddress())),
                    },
                },
            ],
            sender: publishToolbox.keypair.toIotaAddress(),
            commands: [
                {
                    $Intent: {
                        data: {
                            balance: '1',
                            type: testType,
                        },
                        inputs: {},
                        name: 'CoinWithBalance',
                    },
                },
                {
                    TransferObjects: {
                        objects: [
                            {
                                Result: 0,
                            },
                        ],
                        address: {
                            Input: 0,
                        },
                    },
                },
            ],
            version: 2,
        });

        expect(
            JSON.parse(
                await tx.toJSON({
                    supportedIntents: [],
                    client: publishToolbox.client,
                }),
            ),
        ).toEqual({
            expiration: null,
            gasData: {
                budget: null,
                owner: null,
                payment: null,
                price: null,
            },
            inputs: [
                {
                    Pure: {
                        bytes: toB64(fromHEX(receiver.toIotaAddress())),
                    },
                },
                {
                    Object: {
                        ImmOrOwnedObject: expect.anything(),
                    },
                },
                {
                    Pure: {
                        bytes: toB64(bcs.u64().serialize(1).toBytes()),
                    },
                },
            ],
            sender: publishToolbox.keypair.toIotaAddress(),
            commands: [
                {
                    SplitCoins: {
                        coin: {
                            Input: 1,
                        },
                        amounts: [
                            {
                                Input: 2,
                            },
                        ],
                    },
                },
                {
                    TransferObjects: {
                        objects: [{ NestedResult: [0, 0] }],
                        address: {
                            Input: 0,
                        },
                    },
                },
            ],
            version: 2,
        });

        const { digest } = await toolbox.client.signAndExecuteTransaction({
            transaction: tx,
            signer: publishToolbox.keypair,
        });

        const result = await toolbox.client.waitForTransaction({
            digest,
            options: { showEffects: true, showBalanceChanges: true },
        });

        expect(result.effects?.status.status).toBe('success');
        expect(
            result.balanceChanges?.find(
                (change) =>
                    typeof change.owner === 'object' &&
                    'AddressOwner' in change.owner &&
                    change.owner.AddressOwner === receiver.toIotaAddress(),
            ),
        ).toEqual({
            amount: '1',
            coinType: testType,
            owner: {
                AddressOwner: receiver.toIotaAddress(),
            },
        });
    });

    it('works with multiple coins', async () => {
        const tx = new Transaction();
        const receiver = new Ed25519Keypair();

        tx.transferObjects(
            [
                coinWithBalance({ type: testType, balance: 1n }),
                coinWithBalance({ type: testType, balance: 2n }),
                coinWithBalance({ type: 'gas', balance: 3n }),
                coinWithBalance({ type: 'gas', balance: 4n }),
            ],
            receiver.toIotaAddress(),
        );

        tx.setSender(publishToolbox.keypair.toIotaAddress());

        expect(
            JSON.parse(
                await tx.toJSON({
                    supportedIntents: ['CoinWithBalance'],
                }),
            ),
        ).toEqual({
            expiration: null,
            gasData: {
                budget: null,
                owner: null,
                payment: null,
                price: null,
            },
            inputs: [
                {
                    Pure: {
                        bytes: toB64(fromHEX(receiver.toIotaAddress())),
                    },
                },
            ],
            sender: publishToolbox.keypair.toIotaAddress(),
            commands: [
                {
                    $Intent: {
                        data: {
                            balance: '1',
                            type: testType,
                        },
                        inputs: {},
                        name: 'CoinWithBalance',
                    },
                },
                {
                    $Intent: {
                        data: {
                            balance: '2',
                            type: testType,
                        },
                        inputs: {},
                        name: 'CoinWithBalance',
                    },
                },
                {
                    $Intent: {
                        data: {
                            balance: '3',
                            type: 'gas',
                        },
                        inputs: {},
                        name: 'CoinWithBalance',
                    },
                },
                {
                    $Intent: {
                        data: {
                            balance: '4',
                            type: 'gas',
                        },
                        inputs: {},
                        name: 'CoinWithBalance',
                    },
                },
                {
                    TransferObjects: {
                        objects: [
                            {
                                Result: 0,
                            },
                            {
                                Result: 1,
                            },
                            {
                                Result: 2,
                            },
                            {
                                Result: 3,
                            },
                        ],
                        address: {
                            Input: 0,
                        },
                    },
                },
            ],
            version: 2,
        });

        expect(
            JSON.parse(
                await tx.toJSON({
                    supportedIntents: [],
                    client: publishToolbox.client,
                }),
            ),
        ).toEqual({
            expiration: null,
            gasData: {
                budget: null,
                owner: null,
                payment: null,
                price: null,
            },
            inputs: [
                {
                    Pure: {
                        bytes: toB64(fromHEX(receiver.toIotaAddress())),
                    },
                },
                {
                    Object: {
                        ImmOrOwnedObject: expect.anything(),
                    },
                },
                {
                    Pure: {
                        bytes: toB64(bcs.u64().serialize(1).toBytes()),
                    },
                },
                {
                    Pure: {
                        bytes: toB64(bcs.u64().serialize(2).toBytes()),
                    },
                },
                {
                    Pure: {
                        bytes: toB64(bcs.u64().serialize(3).toBytes()),
                    },
                },
                {
                    Pure: {
                        bytes: toB64(bcs.u64().serialize(4).toBytes()),
                    },
                },
            ],
            sender: publishToolbox.keypair.toIotaAddress(),
            commands: [
                {
                    SplitCoins: {
                        coin: {
                            Input: 1,
                        },
                        amounts: [
                            {
                                Input: 2,
                            },
                        ],
                    },
                },
                {
                    SplitCoins: {
                        coin: {
                            Input: 1,
                        },
                        amounts: [
                            {
                                Input: 3,
                            },
                        ],
                    },
                },
                {
                    SplitCoins: {
                        coin: {
                            GasCoin: true,
                        },
                        amounts: [
                            {
                                Input: 4,
                            },
                        ],
                    },
                },
                {
                    SplitCoins: {
                        coin: {
                            GasCoin: true,
                        },
                        amounts: [
                            {
                                Input: 5,
                            },
                        ],
                    },
                },
                {
                    TransferObjects: {
                        objects: [
                            { NestedResult: [0, 0] },
                            { NestedResult: [1, 0] },
                            { NestedResult: [2, 0] },
                            { NestedResult: [3, 0] },
                        ],
                        address: {
                            Input: 0,
                        },
                    },
                },
            ],
            version: 2,
        });

        const { digest } = await toolbox.client.signAndExecuteTransaction({
            transaction: tx,
            signer: publishToolbox.keypair,
        });

        const result = await toolbox.client.waitForTransaction({
            digest,
            options: { showEffects: true, showBalanceChanges: true },
        });

        expect(result.effects?.status.status).toBe('success');
        expect(
            result.balanceChanges?.filter(
                (change) =>
                    typeof change.owner === 'object' &&
                    'AddressOwner' in change.owner &&
                    change.owner.AddressOwner === receiver.toIotaAddress(),
            ),
        ).toEqual([
            {
                amount: '7',
                coinType: '0x2::iota::IOTA',
                owner: {
                    AddressOwner: receiver.toIotaAddress(),
                },
            },
            {
                amount: '3',
                coinType: testType,
                owner: {
                    AddressOwner: receiver.toIotaAddress(),
                },
            },
        ]);
    });
});

// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { beforeAll, beforeEach, describe, expect, it } from 'vitest';

import { Transaction, TransactionResult } from '../../src/transactions';
import { Commands } from '../../src/transactions/Commands';
import type { TransactionDataBuilder } from '../../src/transactions/TransactionData';
import type { BuildTransactionOptions } from '../../src/transactions/json-rpc-resolver.js';
import { normalizeIotaObjectId } from '../../src/utils';
import { publishPackage, setup, TestToolbox } from './utils/setup';

export const IOTA_CLOCK_OBJECT_ID = normalizeIotaObjectId('0x6');

const packagePath = __dirname + '/data/serializer';

describe('TransactionData.insertTransaction', () => {
    let toolbox: TestToolbox;
    let packageId: string;
    let sharedObjectId: string;

    beforeAll(async () => {
        const { packageId: pkgId, publishTxn } = await publishPackage(packagePath);
        packageId = pkgId;

        // Find the shared object created during publish
        const sharedObject = publishTxn.effects?.created!.filter(
            (o) =>
                typeof o.owner === 'object' &&
                'Shared' in o.owner &&
                o.owner.Shared.initial_shared_version !== undefined,
        )[0];
        sharedObjectId = sharedObject!.reference.objectId;
    });

    beforeEach(async () => {
        toolbox = await setup();
    });

    it('should execute merged transaction with shared object (clock) deduplication', async () => {
        const MERGE_INTENT = 'MergeIntent';

        // Transaction plugin that merges placeholder transactions
        async function mergePlaceholderPlugin(
            transactionData: TransactionDataBuilder,
            _buildOptions: BuildTransactionOptions,
            next: () => Promise<void>,
        ) {
            for (let i = transactionData.commands.length - 1; i >= 0; i--) {
                const command = transactionData.commands[i];
                if (command.$kind === '$Intent' && command.$Intent.name === MERGE_INTENT) {
                    const intentData = command.$Intent.data as {
                        transactionToMerge: typeof transactionData;
                    };
                    const otherTxData = intentData.transactionToMerge;

                    transactionData.insertTransaction(i, otherTxData);
                    transactionData.replaceCommand(i + otherTxData.commands.length, []);
                }
            }

            await next();
        }

        // Main transaction that uses the clock
        const mainTx = new Transaction();
        mainTx.addIntentResolver(MERGE_INTENT, mergePlaceholderPlugin);

        // Call use_clock in main transaction
        mainTx.moveCall({
            target: `${packageId}::serializer_tests::use_clock`,
            arguments: [mainTx.object(IOTA_CLOCK_OBJECT_ID)],
        });

        // Create replacement transaction that ALSO uses the same clock
        const replacementTx = new Transaction();
        replacementTx.moveCall({
            target: `${packageId}::serializer_tests::use_clock`,
            arguments: [replacementTx.object(IOTA_CLOCK_OBJECT_ID)],
        });

        await replacementTx.prepareForSerialization({});
        const replacementTxData = replacementTx.getData();

        // Add placeholder intent
        mainTx.add(
            Commands.Intent({
                name: MERGE_INTENT,
                inputs: {},
                data: { transactionToMerge: replacementTxData },
            }),
        );

        // Add another call after the merge that uses clock again
        mainTx.moveCall({
            target: `${packageId}::serializer_tests::use_clock`,
            arguments: [mainTx.object(IOTA_CLOCK_OBJECT_ID)],
        });

        // Execute the merged transaction - should work with deduplicated clock input
        const result = await toolbox.client.signAndExecuteTransaction({
            transaction: mainTx,
            signer: toolbox.keypair,
            options: {
                showEffects: true,
            },
        });

        // Verify execution succeeded
        expect(result.effects?.status.status).toBe('success');
    });

    it('should execute transaction with coin splits merged', async () => {
        const MERGE_INTENT = 'MergeIntent';

        async function mergePlaceholderPlugin(
            transactionData: TransactionDataBuilder,
            _buildOptions: BuildTransactionOptions,
            next: () => Promise<void>,
        ) {
            for (let i = transactionData.commands.length - 1; i >= 0; i--) {
                const command = transactionData.commands[i];
                if (command.$kind === '$Intent' && command.$Intent.name === MERGE_INTENT) {
                    const intentData = command.$Intent.data as {
                        transactionToMerge: typeof transactionData;
                    };
                    const otherTxData = intentData.transactionToMerge;
                    transactionData.insertTransaction(i, otherTxData);
                    transactionData.replaceCommand(i + otherTxData.commands.length, []);
                }
            }

            await next();
        }

        const mainTx = new Transaction();
        mainTx.addIntentResolver(MERGE_INTENT, mergePlaceholderPlugin);

        // Split coins in main transaction
        const [mainCoin] = mainTx.splitCoins(mainTx.gas, [1000]);
        mainTx.transferObjects([mainCoin], toolbox.address());

        // Create replacement transaction with more splits
        const replacementTx = new Transaction();
        const [replacementCoin1, replacementCoin2] = replacementTx.splitCoins(
            replacementTx.gas,
            [2000, 3000],
        );
        replacementTx.transferObjects([replacementCoin1], toolbox.address());
        replacementTx.transferObjects([replacementCoin2], toolbox.address());

        await replacementTx.prepareForSerialization({});

        // Add placeholder
        mainTx.add(
            Commands.Intent({
                name: MERGE_INTENT,
                inputs: {},
                data: { transactionToMerge: replacementTx.getData() },
            }),
        );

        // Execute
        const result = await toolbox.client.signAndExecuteTransaction({
            transaction: mainTx,
            signer: toolbox.keypair,
            options: {
                showEffects: true,
            },
        });

        // Verify success
        expect(result.effects?.status.status).toBe('success');
    });

    it('should deduplicate same coin object used in both transactions', async () => {
        const MERGE_INTENT = 'MergeIntent';

        async function mergePlaceholderPlugin(
            transactionData: TransactionDataBuilder,
            _buildOptions: BuildTransactionOptions,
            next: () => Promise<void>,
        ) {
            for (let i = transactionData.commands.length - 1; i >= 0; i--) {
                const command = transactionData.commands[i];
                if (command.$kind === '$Intent' && command.$Intent.name === MERGE_INTENT) {
                    const intentData = command.$Intent.data as {
                        transactionToMerge: typeof transactionData;
                    };
                    const otherTxData = intentData.transactionToMerge;
                    transactionData.insertTransaction(i, otherTxData);
                    transactionData.replaceCommand(i + otherTxData.commands.length, []);
                }
            }

            await next();
        }

        // Get a coin object
        const coins = await toolbox.getGasObjectsOwnedByAddress();
        const coinObjectId = coins.data[0].coinObjectId;

        const mainTx = new Transaction();
        mainTx.addIntentResolver(MERGE_INTENT, mergePlaceholderPlugin);

        // Use the coin in main transaction
        const [splitCoin1] = mainTx.splitCoins(mainTx.object(coinObjectId), [500]);
        mainTx.transferObjects([splitCoin1], toolbox.address());

        // Create replacement transaction using the SAME coin
        const replacementTx = new Transaction();
        const [splitCoin2] = replacementTx.splitCoins(replacementTx.object(coinObjectId), [600]);
        replacementTx.transferObjects([splitCoin2], toolbox.address());

        await replacementTx.prepareForSerialization({});

        // Add placeholder
        mainTx.add(
            Commands.Intent({
                name: MERGE_INTENT,
                inputs: {},
                data: { transactionToMerge: replacementTx.getData() },
            }),
        );

        // Execute - should work because the coin input is deduplicated
        const result = await toolbox.client.signAndExecuteTransaction({
            transaction: mainTx,
            signer: toolbox.keypair,
            options: {
                showEffects: true,
            },
        });

        // Verify success
        expect(result.effects?.status.status).toBe('success');
    });

    it('should upgrade shared object from immutable to mutable when merging', async () => {
        const MERGE_INTENT = 'MergeIntent';

        async function mergePlaceholderPlugin(
            transactionData: TransactionDataBuilder,
            _buildOptions: BuildTransactionOptions,
            next: () => Promise<void>,
        ) {
            for (let i = transactionData.commands.length - 1; i >= 0; i--) {
                const command = transactionData.commands[i];
                if (command.$kind === '$Intent' && command.$Intent.name === MERGE_INTENT) {
                    const intentData = command.$Intent.data as {
                        transactionToMerge: typeof transactionData;
                    };
                    const otherTxData = intentData.transactionToMerge;
                    transactionData.insertTransaction(i, otherTxData);
                    transactionData.replaceCommand(i + otherTxData.commands.length, []);
                }
            }

            await next();
        }

        const mainTx = new Transaction();
        mainTx.addIntentResolver(MERGE_INTENT, mergePlaceholderPlugin);

        // Main transaction reads the shared object (immutable)
        mainTx.moveCall({
            target: `${packageId}::serializer_tests::value`,
            arguments: [mainTx.object(sharedObjectId)],
        });

        // Replacement transaction writes to the SAME shared object (mutable)
        const replacementTx = new Transaction();
        replacementTx.moveCall({
            target: `${packageId}::serializer_tests::set_value`,
            arguments: [replacementTx.object(sharedObjectId)],
        });

        await replacementTx.prepareForSerialization({});

        // Add placeholder
        mainTx.add(
            Commands.Intent({
                name: MERGE_INTENT,
                inputs: {},
                data: { transactionToMerge: replacementTx.getData() },
            }),
        );

        // Read again after the write
        mainTx.moveCall({
            target: `${packageId}::serializer_tests::value`,
            arguments: [mainTx.object(sharedObjectId)],
        });

        // Execute - should work with the shared object upgraded to mutable
        const result = await toolbox.client.signAndExecuteTransaction({
            transaction: mainTx,
            signer: toolbox.keypair,
            options: {
                showEffects: true,
            },
        });

        // Verify success - proves the mutability upgrade worked
        expect(result.effects?.status.status).toBe('success');
    });

    it('should execute transaction with replaceCommandWithTransaction using NestedResult resultIndex', async () => {
        const PLACEHOLDER = 'PLACEHOLDER';

        async function replacePlugin(
            transactionData: TransactionDataBuilder,
            _buildOptions: BuildTransactionOptions,
            next: () => Promise<void>,
        ) {
            for (let i = transactionData.commands.length - 1; i >= 0; i--) {
                const command = transactionData.commands[i];
                if (command.$kind === '$Intent' && command.$Intent.name === PLACEHOLDER) {
                    const intentData = command.$Intent.data as {
                        transaction: typeof transactionData;
                        resultIndex: TransactionResult;
                    };
                    transactionData.replaceCommandWithTransaction(
                        i,
                        intentData.transaction,
                        intentData.resultIndex,
                    );
                }
            }

            await next();
        }

        const mainTx = new Transaction();
        mainTx.addIntentResolver(PLACEHOLDER, replacePlugin);

        // Create replacement transaction - split coins and use both
        const replacementTx = new Transaction();
        const [coin1] = replacementTx.splitCoins(replacementTx.gas, [500, 1000]);
        // Transfer coin1 so it's not unused
        replacementTx.transferObjects([coin1], toolbox.address());

        await replacementTx.prepareForSerialization({});

        // Add placeholder intent that will be replaced, mapping to NestedResult[0, 1] (coin2)
        const [placeholder] = mainTx.add(
            Commands.Intent({
                name: PLACEHOLDER,
                inputs: {},
                data: {
                    transaction: replacementTx.getData(),
                    resultIndex: {
                        $kind: 'NestedResult',
                        NestedResult: [0, 1],
                    },
                },
            }),
        );
        // Use coin2 from the replacement transaction
        mainTx.transferObjects([placeholder], toolbox.address());

        // Execute
        const result = await toolbox.client.signAndExecuteTransaction({
            transaction: mainTx,
            signer: toolbox.keypair,
            options: {
                showEffects: true,
            },
        });

        // Verify success
        expect(result.effects?.status.status).toBe('success');
    });

    it('should execute transaction with replaceCommandWithTransaction using NestedResult[0,0] mapped to Result', async () => {
        const PLACEHOLDER = 'PLACEHOLDER';

        async function replacePlugin(
            transactionData: TransactionDataBuilder,
            _buildOptions: BuildTransactionOptions,
            next: () => Promise<void>,
        ) {
            for (let i = transactionData.commands.length - 1; i >= 0; i--) {
                const command = transactionData.commands[i];
                if (command.$Intent && command.$Intent.name === PLACEHOLDER) {
                    const intentData = command.$Intent.data as {
                        transaction: typeof transactionData;
                        resultIndex: TransactionResult;
                    };
                    transactionData.replaceCommandWithTransaction(
                        i,
                        intentData.transaction,
                        intentData.resultIndex,
                    );
                }
            }

            await next();
        }

        const mainTx = new Transaction();
        mainTx.addIntentResolver(PLACEHOLDER, replacePlugin);

        // Create replacement transaction - split a single coin
        const replacementTx = new Transaction();
        replacementTx.splitCoins(replacementTx.gas, [2000]);

        await replacementTx.prepareForSerialization({});

        // Add placeholder intent, mapping to NestedResult[0, 0] (the single split coin)
        const [placeholder] = mainTx.add(
            Commands.Intent({
                name: PLACEHOLDER,
                inputs: {},
                data: {
                    transaction: replacementTx.getData(),
                    resultIndex: {
                        $kind: 'NestedResult',
                        NestedResult: [0, 0],
                    },
                },
            }),
        );
        // Transfer the split coin
        mainTx.transferObjects([placeholder], toolbox.address());

        // Execute
        const result2 = await toolbox.client.signAndExecuteTransaction({
            transaction: mainTx,
            signer: toolbox.keypair,
            options: {
                showEffects: true,
            },
        });

        // Verify success
        expect(result2.effects?.status.status).toBe('success');
    });
});

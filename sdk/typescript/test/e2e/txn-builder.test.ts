// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { beforeAll, beforeEach, describe, expect, it } from 'vitest';

import {
    IotaClient,
    IotaObjectChangeCreated,
    IotaTransactionBlockResponse,
    OwnedObjectRef,
} from '../../src/client';
import type { Keypair } from '../../src/cryptography';
import { normalizeIotaObjectId, IOTA_SYSTEM_STATE_OBJECT_ID } from '../../src/utils';
import {
    DEFAULT_GAS_BUDGET,
    DEFAULT_RECIPIENT,
    publishPackage,
    setup,
    TestToolbox,
    upgradePackage,
} from './utils/setup';

import '../../src/transactions/TransactionBlockData';

import { bcs } from '../../src/bcs';
import { TransactionBlock } from '../../src/transactions';

export const IOTA_CLOCK_OBJECT_ID = normalizeIotaObjectId('0x6');

describe('Transaction Builders', () => {
    let toolbox: TestToolbox;
    let packageId: string;
    let publishTxn: IotaTransactionBlockResponse;
    let sharedObjectId: string;

    beforeAll(async () => {
        const packagePath = __dirname + '/./data/serializer';
        ({ packageId, publishTxn } = await publishPackage(packagePath));
        const sharedObject = publishTxn.effects?.created!.find(
            (o) =>
                typeof o.owner === 'object' &&
                'Shared' in o.owner &&
                o.owner.Shared.initial_shared_version !== undefined,
        ) as OwnedObjectRef;
        sharedObjectId = sharedObject.reference.objectId;
    });

    beforeEach(async () => {
        toolbox = await setup();
    });

    it('SplitCoins + TransferObjects', async () => {
        const coins = await toolbox.getGasObjectsOwnedByAddress();
        const tx = new TransactionBlock();
        const coin_0 = coins.data[0];

        const coin = tx.splitCoins(tx.object(coin_0.coinObjectId), [
            bcs.u64().serialize(DEFAULT_GAS_BUDGET * 2),
        ]);
        tx.transferObjects([coin], toolbox.address());
        await validateTransaction(toolbox.client, toolbox.keypair, tx);
    });

    it('MergeCoins', async () => {
        const coins = await toolbox.getGasObjectsOwnedByAddress();
        const [coin_0, coin_1] = coins.data;
        const tx = new TransactionBlock();
        tx.mergeCoins(coin_0.coinObjectId, [coin_1.coinObjectId]);
        await validateTransaction(toolbox.client, toolbox.keypair, tx);
    });

    it('MoveCall', async () => {
        const coins = await toolbox.getGasObjectsOwnedByAddress();
        const [coin_0] = coins.data;
        const tx = new TransactionBlock();
        tx.moveCall({
            target: '0x2::pay::split',
            typeArguments: ['0x2::iota::IOTA'],
            arguments: [tx.object(coin_0.coinObjectId), tx.pure.u64(DEFAULT_GAS_BUDGET * 2)],
        });
        await validateTransaction(toolbox.client, toolbox.keypair, tx);
    });

    it(
        'MoveCall Shared Object',
        async () => {
            const coins = await toolbox.getGasObjectsOwnedByAddress();
            const coin_2 = coins.data[2];

            const [{ iotaAddress: validatorAddress }] = await toolbox.getActiveValidators();

            const tx = new TransactionBlock();
            tx.moveCall({
                target: '0x3::iota_system::request_add_stake',
                arguments: [
                    tx.object(IOTA_SYSTEM_STATE_OBJECT_ID),
                    tx.object(coin_2.coinObjectId),
                    tx.pure.address(validatorAddress),
                ],
            });

            await validateTransaction(toolbox.client, toolbox.keypair, tx);
        },
        {
            // TODO: This test is currently flaky, so adding a retry to unblock merging
            retry: 10,
        },
    );

    it('SplitCoins from gas object + TransferObjects', async () => {
        const tx = new TransactionBlock();
        const coin = tx.splitCoins(tx.gas, [1]);
        tx.transferObjects([coin], DEFAULT_RECIPIENT);
        await validateTransaction(toolbox.client, toolbox.keypair, tx);
    });

    it('TransferObjects gas object', async () => {
        const tx = new TransactionBlock();
        tx.transferObjects([tx.gas], DEFAULT_RECIPIENT);
        await validateTransaction(toolbox.client, toolbox.keypair, tx);
    });

    it('TransferObject', async () => {
        const coins = await toolbox.getGasObjectsOwnedByAddress();
        const tx = new TransactionBlock();
        const coin_0 = coins.data[2];

        tx.transferObjects([coin_0.coinObjectId], DEFAULT_RECIPIENT);
        await validateTransaction(toolbox.client, toolbox.keypair, tx);
    });

    it('Move Shared Object Call with mixed usage of mutable and immutable references', async () => {
        const tx = new TransactionBlock();
        tx.moveCall({
            target: `${packageId}::serializer_tests::value`,
            arguments: [tx.object(sharedObjectId)],
        });
        tx.moveCall({
            target: `${packageId}::serializer_tests::set_value`,
            arguments: [tx.object(sharedObjectId)],
        });
        await validateTransaction(toolbox.client, toolbox.keypair, tx);
    });

    it('Move Shared Object Call by Value', async () => {
        const tx = new TransactionBlock();
        tx.moveCall({
            target: `${packageId}::serializer_tests::value`,
            arguments: [tx.object(sharedObjectId)],
        });
        tx.moveCall({
            target: `${packageId}::serializer_tests::delete_value`,
            arguments: [tx.object(sharedObjectId)],
        });
        await validateTransaction(toolbox.client, toolbox.keypair, tx);
    });

    it('immutable clock', async () => {
        const tx = new TransactionBlock();
        tx.moveCall({
            target: `${packageId}::serializer_tests::use_clock`,
            arguments: [tx.object(IOTA_CLOCK_OBJECT_ID)],
        });
        await validateTransaction(toolbox.client, toolbox.keypair, tx);
    });

    it(
        'Publish and Upgrade Package',
        async () => {
            // Step 1. Publish the package
            const originalPackagePath = __dirname + '/./data/serializer';
            const { packageId, publishTxn } = await publishPackage(originalPackagePath, toolbox);

            const capId = (
                publishTxn.objectChanges?.find(
                    (a) =>
                        a.type === 'created' &&
                        a.objectType.endsWith('UpgradeCap') &&
                        'Immutable' !== a.owner &&
                        'AddressOwner' in a.owner &&
                        a.owner.AddressOwner === toolbox.address(),
                ) as IotaObjectChangeCreated
            )?.objectId;

            expect(capId).toBeTruthy();

            const sharedObjectId = publishTxn.effects?.created!.find(
                (o) =>
                    typeof o.owner === 'object' &&
                    'Shared' in o.owner &&
                    o.owner.Shared.initial_shared_version !== undefined,
            )?.reference.objectId as string;

            // Step 2. Confirm that its functions work as expected in its
            // first version
            const callOrigTx = new TransactionBlock();
            callOrigTx.moveCall({
                target: `${packageId}::serializer_tests::value`,
                arguments: [callOrigTx.object(sharedObjectId)],
            });
            callOrigTx.moveCall({
                target: `${packageId}::serializer_tests::set_value`,
                arguments: [callOrigTx.object(sharedObjectId)],
            });
            await validateTransaction(toolbox.client, toolbox.keypair, callOrigTx);

            // Step 3. Publish the upgrade for the package.
            const upgradedPackagePath = __dirname + '/./data/serializer_upgrade';

            // Step 4. Make sure the behaviour of the upgrade package matches
            // the newly introduced function
            await upgradePackage(packageId, capId, upgradedPackagePath, toolbox);
        },
        {
            // TODO: This test is currently flaky, so adding a retry to unblock merging
            retry: 10,
        },
    );
});

async function validateTransaction(client: IotaClient, signer: Keypair, tx: TransactionBlock) {
    tx.setSenderIfNotSet(signer.getPublicKey().toIotaAddress());
    const localDigest = await tx.getDigest({ client });
    const result = await client.signAndExecuteTransactionBlock({
        signer,
        transactionBlock: tx,
        options: {
            showEffects: true,
        },
    });
    expect(localDigest).toEqual(result.digest);
    expect(result.effects?.status.status).toEqual('success');
}

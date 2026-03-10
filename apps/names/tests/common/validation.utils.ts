// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';

import { adminKeypair, client, iotaNamesClient } from '../setup/utils';

export async function addToDenyList(
    labels: string[],
    labelType: 'blocked' | 'reserved',
    waitForTransaction: boolean,
): Promise<void> {
    const packageId = iotaNamesClient.getPackage('packageId');
    const iotaNamesObjectId = iotaNamesClient.getPackage('iotaNamesObjectId');
    const adminCap = iotaNamesClient.getPackage('adminCap');

    const target = `${packageId}::deny_list::add_${labelType}_labels`;

    const tx = new Transaction();

    tx.moveCall({
        target,
        arguments: [
            tx.object(iotaNamesObjectId),
            tx.object(adminCap),
            tx.pure.vector('string', labels),
        ],
    });

    tx.setSender(adminKeypair.toIotaAddress());

    const resp = await client.signAndExecuteTransaction({
        transaction: await tx.build({
            client: client,
        }),
        signer: adminKeypair,
        options: {
            showEffects: true,
        },
    });

    if (resp.effects?.status.status !== 'success') {
        throw new Error(`Failed to add ${labelType} labels`);
    }

    if (waitForTransaction) {
        await client.waitForTransaction({
            digest: resp.digest,
        });
    }
}

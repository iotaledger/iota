// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { blake2b } from '@noble/hashes/blake2';
import { bytesToHex } from '@noble/hashes/utils';
import { expect } from '@playwright/test';

import { adminKeypair, client, getAuthorizedSmartContractTypes, iotaNamesClient } from './utils';

const envs = {
    COUPONS: iotaNamesClient.getPackage('couponsPackageId'),
    IOTA_NAMES_PACKAGE_ADDRESS: iotaNamesClient.getPackage('packageId'),
    IOTA_NAMES_OBJECT_ID: iotaNamesClient.getPackage('iotaNamesObjectId'),
    ADMIN_CAP: iotaNamesClient.getPackage('adminCap'),
    IOTA_NAMES_PAYMENTS_PACKAGE_ADDRESS: iotaNamesClient.getPackage('paymentsPackageId'),
    ADMIN_ADDRESS: iotaNamesClient.getPackage('adminAddress'),
    IOTA_NAMES_AUCTIONS_PACKAGE_ADDRESS: iotaNamesClient.getPackage('auctionPackageId'),
};

async function sendAuthTransaction(
    authorize: boolean,
    mode: 'payment' | 'auction',
    waitForTransaction: boolean,
): Promise<void> {
    const target = `${envs.IOTA_NAMES_PACKAGE_ADDRESS}::iota_names::${authorize ? 'authorize' : 'deauthorize'}`;

    const typeArg =
        mode === 'payment'
            ? `${envs.IOTA_NAMES_PAYMENTS_PACKAGE_ADDRESS}::payments::PaymentsAuth`
            : `${envs.IOTA_NAMES_AUCTIONS_PACKAGE_ADDRESS}::auction::AuctionAuth`;

    const tx = new Transaction();

    tx.moveCall({
        target,
        arguments: [tx.object(envs.ADMIN_CAP), tx.object(envs.IOTA_NAMES_OBJECT_ID)],
        typeArguments: [typeArg],
    });

    tx.setSender(adminKeypair.toIotaAddress());

    const resp = await client.signAndExecuteTransaction({
        transaction: await tx.build({
            client,
        }),
        signer: adminKeypair,
        options: {
            showEffects: true,
        },
    });

    if (resp.effects?.status.status === 'failure') {
        throw new Error(
            `Failed to ${authorize ? 'authorize' : 'deauthorize'} ${mode} smart contract: ${JSON.stringify(
                resp.effects.status,
            )}`,
        );
    }

    if (waitForTransaction) {
        await client.waitForTransaction({
            digest: resp.digest,
        });
    }
}

export async function toggleSmartContractMode(mode: 'auctions' | 'purchases'): Promise<void> {
    if (!process.env.ADMIN_MNEMONIC) {
        throw new Error('env ADMIN_MNEMONIC not set. Cannot toggle smart contract mode.');
    }

    const targetState = {
        isAuctionAuthorized: mode === 'auctions',
        isPaymentAuthorized: mode === 'purchases',
    };

    const currentState = await getAuthorizedSmartContractTypes();

    if (currentState.isAuctionAuthorized !== targetState.isAuctionAuthorized) {
        await sendAuthTransaction(targetState.isAuctionAuthorized, 'auction', true);
    }

    if (currentState.isPaymentAuthorized !== targetState.isPaymentAuthorized) {
        await sendAuthTransaction(targetState.isPaymentAuthorized, 'payment', true);
    }

    if (
        currentState.isAuctionAuthorized === targetState.isAuctionAuthorized &&
        currentState.isPaymentAuthorized === targetState.isPaymentAuthorized
    ) {
        console.log('Smart contract modes are already in the target state. No changes made.');
        return;
    }

    const finalState = await getAuthorizedSmartContractTypes();

    expect(finalState.isAuctionAuthorized).toBe(targetState.isAuctionAuthorized);
    expect(finalState.isPaymentAuthorized).toBe(targetState.isPaymentAuthorized);
}

export async function addToCouponList(coupon: string): Promise<void> {
    if (!process.env.ADMIN_MNEMONIC) {
        throw new Error('env ADMIN_MNEMONIC not set. Cannot toggle smart contract mode.');
    }

    const target = `${envs.COUPONS}::coupon_house::admin_add_percentage_coupon`;
    const rulesTarget = `${envs.COUPONS}::rules::new_empty_rules`;

    const tx = new Transaction();

    const [rules] = tx.moveCall({
        target: rulesTarget,
        arguments: [],
    });
    const couponCodeHash = bytesToHex(blake2b(coupon, { dkLen: 32 }));

    tx.moveCall({
        target,
        arguments: [
            tx.object(envs.ADMIN_CAP),
            tx.object(envs.IOTA_NAMES_OBJECT_ID),
            tx.pure.string(couponCodeHash),
            tx.pure.u64(100),
            tx.object(rules),
        ],
    });

    tx.setSender(adminKeypair.toIotaAddress());

    const resp = await client.signAndExecuteTransaction({
        transaction: await tx.build({
            client,
        }),
        signer: adminKeypair,
        options: {
            showEffects: true,
        },
    });

    if (resp.effects?.status.status === 'failure') {
        throw new Error(
            `Failed to add coupon ${coupon} to coupon list: ${JSON.stringify(resp.effects.status)}`,
        );
    }

    await client.waitForTransaction({
        digest: resp.digest,
    });
}

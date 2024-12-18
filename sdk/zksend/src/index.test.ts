// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe } from 'node:test';
import { getFullnodeUrl, IotaClient, IotaObjectChange } from '@iota/iota-sdk/client';
import { decodeIotaPrivateKey, Keypair } from '@iota/iota-sdk/cryptography';
import { getFaucetHost, requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';
import { NANOS_PER_IOTA, toBase64 } from '@iota/iota-sdk/utils';
import { beforeAll, expect, test } from 'vitest';

import {
    getSentTransactionsWithLinks,
    listCreatedLinks,
    ZkSendLink,
    ZkSendLinkBuilder,
} from './index.js';

export const DEMO_BEAR_CONFIG = {
    packageId: '0xab8ed19f16874f9b8b66b0b6e325ee064848b1a7fdcb1c2f0478b17ad8574e65',
    type: '0xab8ed19f16874f9b8b66b0b6e325ee064848b1a7fdcb1c2f0478b17ad8574e65::demo_bear::DemoBear',
};

const client = new IotaClient({
    url: getFullnodeUrl('testnet'),
});

// address:  0x8ab2b2a5cfa538db19062b79622abe28f3171c8b8048c5957b01846d57574630
const keypair = Ed25519Keypair.fromSecretKey(
    'iotaprivkey1qz3v0pjxalg3z3p9p6lp4x84y74g0qt2y2q36amvkgfh9zzmm4q66y6ccdz',
);

// Automatically get gas from testnet is not working reliably, manually request gas via discord,
// or uncomment the beforeAll and gas function below
beforeAll(async () => {
    const balance = await client.getBalance({
        owner: keypair.toIotaAddress(),
    });

    if (Number(balance.totalBalance) < Number(NANOS_PER_IOTA) * 0.02) {
        await getIotaFromFaucet(keypair);
    }
}, 30_000);

async function getIotaFromFaucet(keypair: Keypair) {
    const faucetHost = getFaucetHost('testnet');
    const result = await requestIotaFromFaucetV0({
        host: faucetHost,
        recipient: keypair.toIotaAddress(),
    });

    if (result.error) {
        throw new Error(result.error);
    }

    await client.waitForTransaction({
        digest: result.transferredGasObjects[0].transferTxDigest,
    });
}

describe('Contract links', () => {
    test('create and claim link', async () => {
        const link = new ZkSendLinkBuilder({
            client,
            network: 'testnet',
            sender: keypair.toIotaAddress(),
        });

        const bears = await createBears(3);

        for (const bear of bears) {
            link.addClaimableObject(bear.objectId);
        }

        link.addClaimableNanos(100n);

        const linkUrl = link.getLink();

        await link.create({
            signer: keypair,
            waitForTransaction: true,
        });

        const claimLink = await ZkSendLink.fromUrl(linkUrl, {
            network: 'testnet',

            client,
        });

        const claimableAssets = claimLink.assets!;

        expect(claimLink.claimed).toEqual(false);
        expect(claimableAssets.nfts.length).toEqual(3);
        expect(claimableAssets.balances).toMatchInlineSnapshot(`
				[
				  {
				    "amount": 100n,
				    "coinType": "0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA",
				  },
				]
			`);

        const claim = await claimLink.claimAssets(keypair.toIotaAddress());

        const res = await client.waitForTransaction({
            digest: claim.digest,
            options: {
                showObjectChanges: true,
            },
        });

        expect(res.objectChanges?.length).toEqual(
            3 + // bears,
                1 + // coin
                1 + // gas
                1, // bag
        );

        const link2 = await ZkSendLink.fromUrl(linkUrl, {
            network: 'testnet',
        });
        expect(link2.assets?.balances).toEqual(claimLink.assets?.balances);
        expect(link2.assets?.nfts.map((nft) => nft.objectId)).toEqual(
            claimLink.assets?.nfts.map((nft) => nft.objectId),
        );
        expect(link2.claimed).toEqual(true);
    }, 30_000);

    test('regenerate links', async () => {
        const linkKp = new Ed25519Keypair();

        const link = new ZkSendLinkBuilder({
            keypair: linkKp,
            client,
            network: 'testnet',
            sender: keypair.toIotaAddress(),
        });

        const bears = await createBears(3);

        for (const bear of bears) {
            link.addClaimableObject(bear.objectId);
        }

        link.addClaimableNanos(100n);

        const { digest } = await link.create({
            signer: keypair,
            waitForTransaction: true,
        });

        await client.waitForTransaction({ digest });

        const {
            data: [
                {
                    links: [lostLink],
                },
            ],
        } = await getSentTransactionsWithLinks({
            address: keypair.toIotaAddress(),
            network: 'testnet',
        });

        const { url, transaction } = await lostLink.createRegenerateTransaction(
            keypair.toIotaAddress(),
        );

        const result = await client.signAndExecuteTransaction({
            transaction,
            signer: keypair,
            options: {
                showEffects: true,
                showObjectChanges: true,
            },
        });

        await client.waitForTransaction({ digest: result.digest });

        const claimLink = await ZkSendLink.fromUrl(url, {
            network: 'testnet',
        });

        expect(claimLink.assets?.nfts.length).toEqual(3);
        expect(claimLink.assets?.balances).toMatchInlineSnapshot(`
				[
				  {
				    "amount": 100n,
				    "coinType": "0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA",
				  },
				]
			`);

        const claim = await claimLink.claimAssets(keypair.toIotaAddress());

        const res = await client.waitForTransaction({
            digest: claim.digest,
            options: {
                showObjectChanges: true,
            },
        });

        expect(res.objectChanges?.length).toEqual(
            3 + // bears,
                1 + // coin
                1 + // gas
                1, // bag
        );
        const link2 = await ZkSendLink.fromUrl(url, {
            network: 'testnet',
        });
        expect(link2.assets?.balances).toEqual(claimLink.assets?.balances);
        expect(link2.assets?.nfts.map((nft) => nft.objectId)).toEqual(
            claimLink.assets?.nfts.map((nft) => nft.objectId),
        );
        expect(link2.claimed).toEqual(true);
    }, 30_000);

    test('reclaim links', async () => {
        const linkKp = new Ed25519Keypair();

        const link = new ZkSendLinkBuilder({
            keypair: linkKp,
            client,
            network: 'testnet',
            sender: keypair.toIotaAddress(),
        });

        const bears = await createBears(3);

        for (const bear of bears) {
            link.addClaimableObject(bear.objectId);
        }

        link.addClaimableNanos(100n);

        const { digest } = await link.create({
            signer: keypair,
            waitForTransaction: true,
        });

        await client.waitForTransaction({ digest });

        const {
            data: [
                {
                    links: [lostLink],
                },
            ],
        } = await getSentTransactionsWithLinks({
            address: keypair.toIotaAddress(),
            network: 'testnet',
        });

        const { digest: claimDigest } = await lostLink.claimAssets(keypair.toIotaAddress(), {
            reclaim: true,
            sign: async (tx) => (await keypair.signTransaction(tx)).signature,
        });

        const result = await client.waitForTransaction({
            digest: claimDigest,
            options: { showObjectChanges: true, showEffects: true },
        });

        expect(result.objectChanges?.length).toEqual(
            3 + // bears,
                1 + // coin
                1 + // gas
                1, // bag
        );
    }, 30_000);

    test('bulk link creation', async () => {
        const bears = await createBears(3);

        const links = [];
        for (const bear of bears) {
            const link = new ZkSendLinkBuilder({
                client,
                network: 'testnet',
                sender: keypair.toIotaAddress(),
            });

            link.addClaimableNanos(100n);
            link.addClaimableObject(bear.objectId);

            links.push(link);
        }

        const tx = await ZkSendLinkBuilder.createLinks({
            links,
            client,
            network: 'testnet',
        });

        const result = await client.signAndExecuteTransaction({
            transaction: tx,
            signer: keypair,
        });

        await client.waitForTransaction({ digest: result.digest });

        for (const link of links) {
            const linkUrl = link.getLink();

            const claimLink = await ZkSendLink.fromUrl(linkUrl, {
                network: 'testnet',
            });

            const claimableAssets = claimLink.assets!;

            expect(claimLink.claimed).toEqual(false);
            expect(claimableAssets.nfts.length).toEqual(1);
            expect(claimableAssets.balances).toMatchInlineSnapshot(`
					[
					  {
					    "amount": 100n,
					    "coinType": "0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA",
					  },
					]
				`);

            const claim = await claimLink.claimAssets(keypair.toIotaAddress());

            const res = await client.waitForTransaction({
                digest: claim.digest,
                options: {
                    showObjectChanges: true,
                },
            });

            expect(res.objectChanges?.length).toEqual(
                1 + // bears,
                    1 + // coin
                    1 + // gas
                    1, // bag
            );
        }
    }, 60_000);
});

describe('Non contract links', () => {
    test('Links with separate gas coin', async () => {
        const link = new ZkSendLinkBuilder({
            client,
            sender: keypair.toIotaAddress(),
            network: 'testnet',
            contract: null,
        });

        const bears = await createBears(3);

        for (const bear of bears) {
            link.addClaimableObject(bear.objectId);
        }

        link.addClaimableNanos(100n);

        const linkUrl = link.getLink();

        // await link.create({
        // 	signer: keypair,
        // 	waitForTransaction: true,
        // });

        // Balances sometimes not updated even though we wait for the transaction to be indexed
        await new Promise((resolve) => setTimeout(resolve, 3000));

        const claimLink = await ZkSendLink.fromUrl(linkUrl, {
            network: 'testnet',
        });

        expect(claimLink.assets?.nfts.length).toEqual(3);
        expect(claimLink.assets?.balances).toMatchInlineSnapshot(`
					[
					  {
					    "amount": 100n,
					    "coinType": "0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA",
					  },
					]
				`);

        const claimTx = await claimLink.claimAssets(new Ed25519Keypair().toIotaAddress());

        const res = await client.waitForTransaction({
            digest: claimTx.digest,
            options: {
                showObjectChanges: true,
            },
        });

        expect(res.objectChanges?.length).toEqual(
            3 + // bears,
                1 + // coin
                1, // gas
        );

        const link2 = await ZkSendLink.fromUrl(linkUrl, {
            network: 'testnet',
        });
        expect(link2.assets?.balances).toEqual(claimLink.assets?.balances);
        expect(link2.assets?.nfts.map((nft) => nft.objectId)).toEqual(
            claimLink.assets?.nfts.map((nft) => nft.objectId),
        );
        expect(link2.claimed).toEqual(true);
    }, 30_000);

    test('Links with single coin', async () => {
        const linkKp = new Ed25519Keypair();

        const tx = new Transaction();

        const [coin] = tx.splitCoins(tx.gas, [5_000_000]);
        tx.transferObjects([coin], linkKp.toIotaAddress());

        const { digest } = await client.signAndExecuteTransaction({
            signer: keypair,
            transaction: tx,
        });

        await client.waitForTransaction({ digest });

        const claimLink = new ZkSendLink({
            keypair: linkKp,
            network: 'testnet',
            isContractLink: false,
        });

        await claimLink.loadAssets();

        expect(claimLink.assets?.nfts.length).toEqual(0);
        expect(claimLink.assets?.balances.length).toEqual(1);
        expect(claimLink.assets?.balances[0].coinType).toEqual(
            '0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA',
        );

        const claimTx = await claimLink.claimAssets(keypair.toIotaAddress());

        const res = await client.waitForTransaction({
            digest: claimTx.digest,
            options: {
                showBalanceChanges: true,
            },
        });

        expect(res.balanceChanges?.length).toEqual(2);
        const link2 = await ZkSendLink.fromUrl(
            `https://zksend.con/claim#${toBase64(decodeIotaPrivateKey(linkKp.getSecretKey()).secretKey)}`,
            {
                network: 'testnet',
            },
        );
        expect(link2.assets?.balances).toEqual(claimLink.assets?.balances);
        expect(link2.assets?.nfts.map((nft) => nft.objectId)).toEqual(
            claimLink.assets?.nfts.map((nft) => nft.objectId),
        );
        expect(link2.claimed).toEqual(true);
    }, 30_000);

    test('Send to address', async () => {
        const link = new ZkSendLinkBuilder({
            client,
            sender: keypair.toIotaAddress(),
            network: 'testnet',
            contract: null,
        });

        const bears = await createBears(3);

        for (const bear of bears) {
            link.addClaimableObject(bear.objectId);
        }

        link.addClaimableNanos(100n);

        const receiver = new Ed25519Keypair();

        const tx = await link.createSendToAddressTransaction({
            address: receiver.toIotaAddress(),
        });

        const { digest } = await client.signAndExecuteTransaction({
            transaction: tx,
            signer: keypair,
        });

        await client.waitForTransaction({
            digest,
        });

        const objects = await client.getOwnedObjects({
            owner: receiver.toIotaAddress(),
        });

        expect(objects.data.length).toEqual(4);
    }, 30_000);

    test('create link with minted assets', async () => {
        const link = new ZkSendLinkBuilder({
            client,
            network: 'testnet',
            sender: keypair.toIotaAddress(),
        });

        const tx = new Transaction();

        for (let i = 0; i < 3; i++) {
            const bear = tx.moveCall({
                target: `${DEMO_BEAR_CONFIG.packageId}::demo_bear::new`,
                arguments: [
                    tx.pure.string(`A happy bear - ${Math.floor(Math.random() * 1_000_000_000)}`),
                ],
            });

            link.addClaimableObjectRef(bear, DEMO_BEAR_CONFIG.type);
        }

        link.addClaimableNanos(100n);

        const linkUrl = link.getLink();

        await link.create({
            transaction: tx,
            signer: keypair,
            waitForTransaction: true,
        });

        const claimLink = await ZkSendLink.fromUrl(linkUrl, {
            network: 'testnet',
        });

        const claimableAssets = claimLink.assets!;

        expect(claimLink.claimed).toEqual(false);
        expect(claimableAssets.nfts.length).toEqual(3);
        expect(claimableAssets.balances).toMatchInlineSnapshot(`
				[
				  {
				    "amount": 100n,
				    "coinType": "0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA",
				  },
				]
			`);

        const claim = await claimLink.claimAssets(keypair.toIotaAddress());

        const res = await client.waitForTransaction({
            digest: claim.digest,
            options: {
                showObjectChanges: true,
            },
        });

        expect(res.objectChanges?.length).toEqual(
            3 + // bears,
                1 + // coin
                1 + // gas
                1, // bag
        );

        const link2 = await ZkSendLink.fromUrl(linkUrl, {
            network: 'testnet',
        });
        expect(link2.assets?.balances).toEqual(claimLink.assets?.balances);
        expect(link2.assets?.nfts.map((nft) => nft.objectId).sort()).toEqual(
            claimLink.assets?.nfts.map((nft) => nft.objectId).sort(),
        );
        expect(link2.claimed).toEqual(true);
    }, 30_000);

    test('list created links', async () => {
        const link = new ZkSendLinkBuilder({
            client,
            network: 'testnet',
            sender: keypair.toIotaAddress(),
        });

        const bears = await createBears(3);

        for (const bear of bears) {
            link.addClaimableObject(bear.objectId);
        }

        link.addClaimableNanos(100n);

        const linkUrl = link.getLink();

        // await link.create({
        // 	signer: keypair,
        // 	waitForTransaction: true,
        // });

        // wait for graphql indexing
        await new Promise((resolve) => setTimeout(resolve, 3000));

        const createdLinks = await listCreatedLinks({
            network: 'testnet',
            address: keypair.toIotaAddress(),
        });

        expect(createdLinks.links[0]?.link.address).toEqual(link.keypair.toIotaAddress());

        expect(createdLinks.links[0].claimed).toEqual(false);
        expect(createdLinks.links[0].assets).toMatchObject({
            balances: [
                {
                    coinType:
                        '0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA',
                    amount: 100n,
                },
            ],
            nfts: [expect.any(Object), expect.any(Object), expect.any(Object)],
        });

        const claimLink = await ZkSendLink.fromUrl(linkUrl, {
            network: 'testnet',

            client,
        });

        const claimableAssets = claimLink.assets!;

        expect(claimLink.claimed).toEqual(false);
        expect(claimableAssets.nfts.length).toEqual(3);
        expect(claimableAssets.balances).toMatchInlineSnapshot(`
				[
				  {
				    "amount": 100n,
				    "coinType": "0x0000000000000000000000000000000000000000000000000000000000000002::iota::IOTA",
				  },
				]
			`);

        const claim = await claimLink.claimAssets(keypair.toIotaAddress());

        const res = await client.waitForTransaction({
            digest: claim.digest,
            options: {
                showObjectChanges: true,
            },
        });

        expect(res.objectChanges?.length).toEqual(
            3 + // bears,
                1 + // coin
                1 + // gas
                1, // bag
        );

        const link2 = await ZkSendLink.fromUrl(linkUrl, {
            network: 'testnet',
        });
        expect(link2.assets?.balances).toEqual(claimLink.assets?.balances);
        expect(link2.assets?.nfts.map((nft) => nft.objectId)).toEqual(
            claimLink.assets?.nfts.map((nft) => nft.objectId),
        );
        expect(link2.claimed).toEqual(true);
    }, 30_000);
});

async function createBears(totalBears: number) {
    const tx = new Transaction();
    const bears = [];

    for (let i = 0; i < totalBears; i++) {
        const bear = tx.moveCall({
            target: `${DEMO_BEAR_CONFIG.packageId}::demo_bear::new`,
            arguments: [
                tx.pure.string(`A happy bear - ${Math.floor(Math.random() * 1_000_000_000)}`),
            ],
        });

        bears.push(bear);
    }

    tx.transferObjects(bears, tx.pure.address(keypair.toIotaAddress()));

    const res = await client.signAndExecuteTransaction({
        transaction: tx,
        signer: keypair,
        options: {
            showObjectChanges: true,
        },
    });

    await client.waitForTransaction({
        digest: res.digest,
    });

    const bearList = res
        .objectChanges!.filter(
            (x: IotaObjectChange) =>
                x.type === 'created' && x.objectType.includes(DEMO_BEAR_CONFIG.type),
        )
        .map((x: IotaObjectChange) => {
            if (!('objectId' in x)) throw new Error('invalid data');
            return {
                objectId: x.objectId,
                type: x.objectType,
            };
        });

    return bearList;
}

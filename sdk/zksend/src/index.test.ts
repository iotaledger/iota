// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getFullnodeUrl, IotaClient, IotaObjectChange } from '@iota/iota.js/client';
import { decodeIotaPrivateKey } from '@iota/iota.js/cryptography';
// import { getFaucetHost, requestIotaFromFaucetV0 } from '@iota/iota.js/faucet';
import { Ed25519Keypair } from '@iota/iota.js/keypairs/ed25519';
import { TransactionBlock } from '@iota/iota.js/transactions';
import { toB64 } from '@iota/iota.js/utils';
import { describe } from 'node:test';
import { expect, test } from 'vitest';

import { ZkSendLink, ZkSendLinkBuilder } from './index.js';
import { listCreatedLinks } from './links/list-created-links.js';

export const DEMO_BEAR_CONFIG = {
    packageId: '0xab8ed19f16874f9b8b66b0b6e325ee064848b1a7fdcb1c2f0478b17ad8574e65',
    type: '0xab8ed19f16874f9b8b66b0b6e325ee064848b1a7fdcb1c2f0478b17ad8574e65::demo_bear::DemoBear',
};

export const ZK_BAG_CONFIG = {
    packageId: '0x036fee67274d0d85c3532f58296abe0dee86b93864f1b2b9074be6adb388f138',
    bagStoreId: '0x5c63e71734c82c48a3cb9124c54001d1a09736cfb1668b3b30cd92a96dd4d0ce',
    bagStoreTableId: '0x4e1bc4085d64005e03eb4eab2510d527aeba9548cda431cb8f149ff37451f870',
};

const client = new IotaClient({
    url: getFullnodeUrl('testnet'),
});

// 0x6e43d0e58341db532a87a16aaa079ae6eb1ed3ae8b77fdfa4870a268ea5d5db8
const keypair = Ed25519Keypair.fromSecretKey(
    decodeIotaPrivateKey('iotaprivkey1qrlgsqryjmmt59nw7a76myeeadxrs3esp8ap2074qz8xaq5kens32f7e3u7')
        .secretKey,
);

// Automatically get gas from testnet is not working reliably, manually request gas via discord,
// or uncomment the beforeAll and gas function below
// beforeAll(async () => {
// 	await getIotaFromFaucet(keypair);
// });

// async function getIotaFromFaucet(keypair: Keypair) {
// 	const faucetHost = getFaucetHost('testnet');
// 	const result = await requestIotaFromFaucetV0({
// 		host: faucetHost,
// 		recipient: keypair.toIotaAddress(),
// 	});

// 	if (result.error) {
// 		throw new Error(result.error);
// 	}
// }

describe('Contract links', () => {
    test(
        'create and claim link',
        async () => {
            const link = new ZkSendLinkBuilder({
                client,
                contract: ZK_BAG_CONFIG,
                sender: keypair.toIotaAddress(),
            });

            const bears = await createBears(3);

            for (const bear of bears) {
                link.addClaimableObject(bear.objectId);
            }

            link.addClaimableMicros(100n);

            const linkUrl = link.getLink();

            await link.create({
                signer: keypair,
                waitForTransactionBlock: true,
            });

            const claimLink = await ZkSendLink.fromUrl(linkUrl, {
                contract: ZK_BAG_CONFIG,
                network: 'testnet',
                claimApi: 'https://zksend-git-mh-contract-claims-mysten-labs.vercel.app/api',
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

            const res = await client.waitForTransactionBlock({
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
                contract: ZK_BAG_CONFIG,
                network: 'testnet',
                claimApi: 'https://zksend-git-mh-contract-claims-mysten-labs.vercel.app/api',
            });
            expect(link2.assets?.balances).toEqual(claimLink.assets?.balances);
            expect(link2.assets?.nfts.map((nft) => nft.objectId)).toEqual(
                claimLink.assets?.nfts.map((nft) => nft.objectId),
            );
            expect(link2.claimed).toEqual(true);
        },
        {
            timeout: 30_000,
        },
    );

    test(
        'regenerate links',
        async () => {
            const linkKp = new Ed25519Keypair();
            const link = new ZkSendLinkBuilder({
                keypair: linkKp,
                client,
                contract: ZK_BAG_CONFIG,
                sender: keypair.toIotaAddress(),
            });

            const bears = await createBears(3);

            for (const bear of bears) {
                link.addClaimableObject(bear.objectId);
            }

            link.addClaimableMicros(100n);

            await link.create({
                signer: keypair,
                waitForTransactionBlock: true,
            });

            const {
                links: [lostLink],
            } = await listCreatedLinks({
                address: keypair.toIotaAddress(),
                network: 'testnet',
                contract: ZK_BAG_CONFIG,
            });

            const { url, transactionBlock } = await lostLink.link.createRegenerateTransaction(
                keypair.toIotaAddress(),
            );

            const result = await client.signAndExecuteTransactionBlock({
                transactionBlock,
                signer: keypair,
                options: {
                    showEffects: true,
                    showObjectChanges: true,
                },
            });

            await client.waitForTransactionBlock({ digest: result.digest });

            const claimLink = await ZkSendLink.fromUrl(url, {
                contract: ZK_BAG_CONFIG,
                network: 'testnet',
                claimApi: 'https://zksend-git-mh-contract-claims-mysten-labs.vercel.app/api',
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

            const res = await client.waitForTransactionBlock({
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
                contract: ZK_BAG_CONFIG,
                network: 'testnet',
                claimApi: 'https://zksend-git-mh-contract-claims-mysten-labs.vercel.app/api',
            });
            expect(link2.assets?.balances).toEqual(claimLink.assets?.balances);
            expect(link2.assets?.nfts.map((nft) => nft.objectId)).toEqual(
                claimLink.assets?.nfts.map((nft) => nft.objectId),
            );
            expect(link2.claimed).toEqual(true);
        },
        {
            timeout: 30_000,
        },
    );

    test(
        'bulk link creation',
        async () => {
            const bears = await createBears(3);

            const links = [];
            for (const bear of bears) {
                const link = new ZkSendLinkBuilder({
                    client,
                    contract: ZK_BAG_CONFIG,
                    sender: keypair.toIotaAddress(),
                });

                link.addClaimableMicros(100n);
                link.addClaimableObject(bear.objectId);

                links.push(link);
            }

            const txb = await ZkSendLinkBuilder.createLinks({
                links,
                client,
                contract: ZK_BAG_CONFIG,
            });

            const result = await client.signAndExecuteTransactionBlock({
                transactionBlock: txb,
                signer: keypair,
            });

            await client.waitForTransactionBlock({ digest: result.digest });

            for (const link of links) {
                const linkUrl = link.getLink();

                const claimLink = await ZkSendLink.fromUrl(linkUrl, {
                    contract: ZK_BAG_CONFIG,
                    network: 'testnet',
                    claimApi: 'https://zksend-git-mh-contract-claims-mysten-labs.vercel.app/api',
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

                const res = await client.waitForTransactionBlock({
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
        },
        {
            timeout: 60_000,
        },
    );
});

describe('Non contract links', () => {
    test(
        'Links with separate gas coin',
        async () => {
            const link = new ZkSendLinkBuilder({
                client,
                sender: keypair.toIotaAddress(),
                contract: null,
            });

            const bears = await createBears(3);

            for (const bear of bears) {
                link.addClaimableObject(bear.objectId);
            }

            link.addClaimableMicros(100n);

            const linkUrl = link.getLink();

            await link.create({
                signer: keypair,
                waitForTransactionBlock: true,
            });

            // Balances sometimes not updated even though we wait for the transaction to be indexed
            await new Promise((resolve) => setTimeout(resolve, 3000));

            const claimLink = await ZkSendLink.fromUrl(linkUrl, {
                contract: ZK_BAG_CONFIG,
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

            const res = await client.waitForTransactionBlock({
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
                contract: ZK_BAG_CONFIG,
                network: 'testnet',
                claimApi: 'https://zksend-git-mh-contract-claims-mysten-labs.vercel.app/api',
            });
            expect(link2.assets?.balances).toEqual(claimLink.assets?.balances);
            expect(link2.assets?.nfts.map((nft) => nft.objectId)).toEqual(
                claimLink.assets?.nfts.map((nft) => nft.objectId),
            );
            expect(link2.claimed).toEqual(true);
        },
        {
            timeout: 30_000,
        },
    );

    test(
        'Links with single coin',
        async () => {
            const linkKp = new Ed25519Keypair();

            const txb = new TransactionBlock();

            const [coin] = txb.splitCoins(txb.gas, [5_000_000]);
            txb.transferObjects([coin], linkKp.toIotaAddress());

            const { digest } = await client.signAndExecuteTransactionBlock({
                signer: keypair,
                transactionBlock: txb,
            });

            await client.waitForTransactionBlock({ digest });

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

            const res = await client.waitForTransactionBlock({
                digest: claimTx.digest,
                options: {
                    showBalanceChanges: true,
                },
            });

            expect(res.balanceChanges?.length).toEqual(2);
            const link2 = await ZkSendLink.fromUrl(
                `https://zksend.con/claim#${toB64(
                    decodeIotaPrivateKey(linkKp.getSecretKey()).secretKey,
                )}`,
                {
                    contract: ZK_BAG_CONFIG,
                    network: 'testnet',
                    claimApi: 'https://zksend-git-mh-contract-claims-mysten-labs.vercel.app/api',
                },
            );
            expect(link2.assets?.balances).toEqual(claimLink.assets?.balances);
            expect(link2.assets?.nfts.map((nft) => nft.objectId)).toEqual(
                claimLink.assets?.nfts.map((nft) => nft.objectId),
            );
            expect(link2.claimed).toEqual(true);
        },
        {
            timeout: 30_000,
        },
    );
});

async function createBears(totalBears: number) {
    const txb = new TransactionBlock();
    const bears = [];

    for (let i = 0; i < totalBears; i++) {
        const bear = txb.moveCall({
            target: `${DEMO_BEAR_CONFIG.packageId}::demo_bear::new`,
            arguments: [
                txb.pure.string(`A happy bear - ${Math.floor(Math.random() * 1_000_000_000)}`),
            ],
        });

        bears.push(bear);
    }

    txb.transferObjects(bears, txb.pure.address(keypair.toIotaAddress()));

    const res = await client.signAndExecuteTransactionBlock({
        transactionBlock: txb,
        signer: keypair,
        options: {
            showObjectChanges: true,
        },
    });

    await client.waitForTransactionBlock({
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

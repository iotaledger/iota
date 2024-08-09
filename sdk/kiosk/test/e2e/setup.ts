// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { execSync } from 'child_process';
import type {
    DevInspectResults,
    IotaObjectChangePublished,
    IotaTransactionBlockResponse,
} from '@iota/iota-sdk/client';
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import {
    FaucetRateLimitError,
    getFaucetHost,
    requestIotaFromFaucetV0,
} from '@iota/iota-sdk/faucet';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { TransactionBlock } from '@iota/iota-sdk/transactions';
import tmp from 'tmp';
import { retry } from 'ts-retry-promise';
import { expect } from 'vitest';

import type { KioskClient } from '../../src/index.js';
import { KioskTransaction } from '../../src/index.js';

//@ts-expect-error env not found on meta
const DEFAULT_FAUCET_URL = import.meta.env.VITE_FAUCET_URL ?? getFaucetHost('localnet');
//@ts-expect-error env not found on meta
const DEFAULT_FULLNODE_URL = import.meta.env.VITE_FULLNODE_URL ?? getFullnodeUrl('localnet');
//@ts-expect-error env not found on meta
const IOTA_BIN = import.meta.env.VITE_IOTA_BIN ?? 'cargo run --bin iota';

export class TestToolbox {
    keypair: Ed25519Keypair;
    client: IotaClient;

    constructor(keypair: Ed25519Keypair, client: IotaClient) {
        this.keypair = keypair;
        this.client = client;
    }

    address() {
        return this.keypair.getPublicKey().toIotaAddress();
    }

    public async getActiveValidators() {
        return (await this.client.getLatestIotaSystemState()).activeValidators;
    }
}

export function getClient(): IotaClient {
    return new IotaClient({
        url: DEFAULT_FULLNODE_URL,
    });
}

// TODO: expose these testing utils from @iota/iota-sdk
export async function setupIotaClient() {
    const keypair = Ed25519Keypair.generate();
    const address = keypair.getPublicKey().toIotaAddress();
    const client = getClient();
    await retry(() => requestIotaFromFaucetV0({ host: DEFAULT_FAUCET_URL, recipient: address }), {
        backoff: 'EXPONENTIAL',
        // overall timeout in 60 seconds
        timeout: 1000 * 60,
        // skip retry if we hit the rate-limit error
        retryIf: (error: any) => !(error instanceof FaucetRateLimitError),
        logger: (msg) => console.warn('Retrying requesting from faucet: ' + msg),
    });
    return new TestToolbox(keypair, client);
}

// TODO: expose these testing utils from @iota/iota-sdk
export async function publishPackage(packagePath: string, toolbox?: TestToolbox) {
    // TODO: We create a unique publish address per publish, but we really could share one for all publishes.
    if (!toolbox) {
        toolbox = await setupIotaClient();
    }

    // remove all controlled temporary objects on process exit
    tmp.setGracefulCleanup();

    const tmpobj = tmp.dirSync({ unsafeCleanup: true });

    const { modules, dependencies } = JSON.parse(
        execSync(
            `${IOTA_BIN} move build --dump-bytecode-as-base64 --path ${packagePath} --install-dir ${tmpobj.name}`,
            { encoding: 'utf-8' },
        ),
    );
    const tx = new TransactionBlock();
    const cap = tx.publish({
        modules,
        dependencies,
    });

    // Transfer the upgrade capability to the sender so they can upgrade the package later if they want.
    tx.transferObjects([cap], tx.pure(await toolbox.address()));

    const publishTxn = await toolbox.client.signAndExecuteTransactionBlock({
        transactionBlock: tx,
        signer: toolbox.keypair,
        options: {
            showEffects: true,
            showObjectChanges: true,
        },
    });
    expect(publishTxn.effects?.status.status).toEqual('success');

    const packageId = ((publishTxn.objectChanges?.filter(
        (a) => a.type === 'published',
    ) as IotaObjectChangePublished[]) ?? [])[0].packageId.replace(/^(0x)(0+)/, '0x') as string;

    expect(packageId).toBeTypeOf('string');

    console.info(`Published package ${packageId} from address ${toolbox.address()}}`);

    return { packageId, publishTxn };
}

export async function publishExtensionsPackage(toolbox: TestToolbox): Promise<string> {
    const packagePath = __dirname + '/../../../../kiosk';
    const { packageId } = await publishPackage(packagePath, toolbox);

    return packageId;
}

export async function publishHeroPackage(toolbox: TestToolbox): Promise<string> {
    const packagePath = __dirname + '/./data/hero';
    const { packageId } = await publishPackage(packagePath, toolbox);

    return packageId;
}

export function print(item: any) {
    console.dir(item, { depth: null });
}

export async function mintHero(toolbox: TestToolbox, packageId: string): Promise<string> {
    const txb = new TransactionBlock();
    const hero = txb.moveCall({
        target: `${packageId}::hero::mint_hero`,
    });
    txb.transferObjects([hero], txb.pure(await toolbox.address(), 'address'));

    const res = await executeTransactionBlock(toolbox, txb);

    return getCreatedObjectIdByType(res, 'hero::Hero');
}

export async function mintVillain(toolbox: TestToolbox, packageId: string): Promise<string> {
    const txb = new TransactionBlock();
    const hero = txb.moveCall({
        target: `${packageId}::hero::mint_villain`,
    });
    txb.transferObjects([hero], txb.pure(await toolbox.address(), 'address'));

    const res = await executeTransactionBlock(toolbox, txb);

    return getCreatedObjectIdByType(res, 'hero::Villain');
}

// create a non-personal kiosk.
export async function createKiosk(toolbox: TestToolbox, kioskClient: KioskClient) {
    const txb = new TransactionBlock();

    new KioskTransaction({ transactionBlock: txb, kioskClient }).createAndShare(toolbox.address());

    await executeTransactionBlock(toolbox, txb);
}

// Create a personal Kiosk.
export async function createPersonalKiosk(toolbox: TestToolbox, kioskClient: KioskClient) {
    const txb = new TransactionBlock();
    new KioskTransaction({ transactionBlock: txb, kioskClient }).createPersonal().finalize();

    await executeTransactionBlock(toolbox, txb);
}

function getCreatedObjectIdByType(res: IotaTransactionBlockResponse, type: string): string {
    return res.objectChanges?.filter(
        (x) => x.type === 'created' && x.objectType.endsWith(type),
        //@ts-expect-error Silence the error here
    )[0].objectId;
}

export async function getPublisherObject(toolbox: TestToolbox): Promise<string> {
    const res = await toolbox.client.getOwnedObjects({
        filter: {
            StructType: '0x2::package::Publisher',
        },
        owner: toolbox.address(),
    });

    const publisherObj = res.data[0].data?.objectId;
    expect(publisherObj).not.toBeUndefined();

    return publisherObj ?? '';
}

export async function executeTransactionBlock(
    toolbox: TestToolbox,
    txb: TransactionBlock,
): Promise<IotaTransactionBlockResponse> {
    const resp = await toolbox.client.signAndExecuteTransactionBlock({
        signer: toolbox.keypair,
        transactionBlock: txb,
        options: {
            showEffects: true,
            showEvents: true,
            showObjectChanges: true,
        },
    });
    expect(resp.effects?.status.status).toEqual('success');
    return resp;
}

export async function devInspectTransactionBlock(
    toolbox: TestToolbox,
    txb: TransactionBlock,
): Promise<DevInspectResults> {
    return await toolbox.client.devInspectTransactionBlock({
        transactionBlock: txb,
        sender: toolbox.address(),
    });
}

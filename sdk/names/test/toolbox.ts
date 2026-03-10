// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { execSync } from 'child_process';
import { mkdtemp } from 'fs/promises';
import { tmpdir } from 'os';
import path from 'path';
import { IotaClientGraphQLTransport } from '@iota/graphql-transport';
import { getNetwork, IotaClient } from '@iota/iota-sdk/client';
import {
    FaucetRateLimitError,
    getFaucetHost,
    requestIotaFromFaucetV0,
} from '@iota/iota-sdk/faucet';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { retry } from 'ts-retry-promise';

//@ts-ignore-next-line
export const IOTA_BIN = process.env.VITE_IOTA_BIN ?? `iota`;

//@ts-ignore-next-line
const DEFAULT_FAUCET_URL = process.env.VITE_FAUCET_URL ?? getFaucetHost('localnet');
//@ts-ignore-next-line
const DEFAULT_GRAPHQL_URL = process.env.VITE_GRAPHQL_URL ?? getNetwork('localnet').graphql!;

export class TestToolbox {
    keypair: Ed25519Keypair;
    client: IotaClient;
    configPath: string;

    constructor(keypair: Ed25519Keypair, client: IotaClient, configPath: string) {
        this.keypair = keypair;
        this.client = client;
        this.configPath = configPath;
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
        transport: new IotaClientGraphQLTransport({ url: DEFAULT_GRAPHQL_URL }),
    });
}

export function getGraphQLClient(): IotaGraphQLClient {
    return new IotaGraphQLClient({
        url: DEFAULT_GRAPHQL_URL,
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

    const b = await retry(
        () =>
            client.getBalance({
                owner: address,
            }),
        {
            retries: 100,
            delay: 50,
            logger: (msg) => console.warn('Retrying getting balance: ' + msg),
            retryIf: (error: any) => error.message.includes('Missing response data'),
        },
    );
    console.log(`Balance for ${address}: ${b.totalBalance} IOTA`);

    const tmpDirPath = path.join(tmpdir(), 'config-');
    const tmpDir = await mkdtemp(tmpDirPath);
    const configPath = path.join(tmpDir, 'client.yaml');
    execSync(`${IOTA_BIN} client --yes --client.config ${configPath}`, { encoding: 'utf-8' });
    return new TestToolbox(keypair, client, configPath);
}

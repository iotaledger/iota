// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import networkEnv from '_src/background/networkEnv';
import { type NetworkEnvType, SentryHttpTransport } from '@iota/core';
import { getNetwork, Network, IotaClient, IotaHTTPTransport } from '@iota/iota-sdk/client';

const iotaClientPerNetwork = new Map<string, IotaClient>();
const SENTRY_MONITORED_ENVS = [Network.Testnet]; // Sentry dev hint: change this to eg [Network.Localnet]

export function getIotaClient({ network, customRpcUrl }: NetworkEnvType): IotaClient {
    const key = `${network}_${customRpcUrl}`;
    if (!iotaClientPerNetwork.has(key)) {
        const connection = getNetwork(network)?.url ?? customRpcUrl;
        if (!connection) {
            throw new Error(`API url not found for network ${network} ${customRpcUrl}`);
        }
        iotaClientPerNetwork.set(
            key,
            new IotaClient({
                transport:
                    !customRpcUrl && SENTRY_MONITORED_ENVS.includes(network)
                        ? new SentryHttpTransport(connection)
                        : new IotaHTTPTransport({ url: connection }),
            }),
        );
    }
    return iotaClientPerNetwork.get(key)!;
}

export async function getActiveNetworkIotaClient(): Promise<IotaClient> {
    return getIotaClient(await networkEnv.getActiveNetwork());
}

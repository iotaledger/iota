// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type NetworkEnvType, createSentryRequestInspector } from '@iota/core';
import { getNetwork, Network, IotaClient, IotaHTTPTransport } from '@iota/iota-sdk/client';

const iotaClientPerNetwork = new Map<string, IotaClient>();
const SENTRY_MONITORED_ENVS = [Network.Mainnet]; // Sentry dev hint: change this to eg [Network.Localnet]

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
                transport: new IotaHTTPTransport({
                    url: connection,
                    inspector:
                        !customRpcUrl && SENTRY_MONITORED_ENVS.includes(network)
                            ? createSentryRequestInspector(connection)
                            : undefined,
                }),
            }),
        );
    }
    return iotaClientPerNetwork.get(key)!;
}

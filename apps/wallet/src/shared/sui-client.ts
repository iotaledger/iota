// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import networkEnv from '_src/background/NetworkEnv';
import { type NetworkEnvType } from '_src/shared/api-env';
import { SentryHttpTransport } from '@mysten/core';
import { getNetwork, Network, SuiClient, SuiHTTPTransport } from '@mysten/sui.js/client';

const suiClientPerNetwork = new Map<string, SuiClient>();
const SENTRY_MONITORED_ENVS = [Network.Custom];

export function getSuiClient({ network, customRpcUrl }: NetworkEnvType): SuiClient {
    const key = `${network}_${customRpcUrl}`;
    if (!suiClientPerNetwork.has(key)) {
        const connection = getNetwork(network)?.url ?? customRpcUrl;
        if (!connection) {
            throw new Error(`API url not found for network ${network} ${customRpcUrl}`);
        }
        suiClientPerNetwork.set(
            key,
            new SuiClient({
                transport:
                    !customRpcUrl && SENTRY_MONITORED_ENVS.includes(network)
                        ? new SentryHttpTransport(connection)
                        : new SuiHTTPTransport({ url: connection }),
            }),
        );
    }
    return suiClientPerNetwork.get(key)!;
}

export async function getActiveNetworkSuiClient(): Promise<SuiClient> {
    return getSuiClient(await networkEnv.getActiveNetwork());
}

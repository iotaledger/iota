// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import networkEnv from '_src/background/NetworkEnv';
import { API_ENV, ENV_TO_API, type NetworkEnvType } from '_src/shared/api-env';
import { SentryHttpTransport } from '@iota/core';
import { IotaClient, IotaHTTPTransport } from '@iota/iota-sdk/client';

const iotaClientPerNetwork = new Map<string, IotaClient>();
const SENTRY_MONITORED_ENVS = [API_ENV.mainnet];

export function getIotaClient({ env, customRpcUrl }: NetworkEnvType): IotaClient {
    const key = `${env}_${customRpcUrl}`;
    if (!iotaClientPerNetwork.has(key)) {
        const connection = customRpcUrl ? customRpcUrl : ENV_TO_API[env];
        if (!connection) {
            throw new Error(`API url not found for network env ${env} ${customRpcUrl}`);
        }
        iotaClientPerNetwork.set(
            key,
            new IotaClient({
                transport:
                    !customRpcUrl && SENTRY_MONITORED_ENVS.includes(env)
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

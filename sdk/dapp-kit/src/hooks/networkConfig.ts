// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaClientOptions } from '@iota/iota-sdk/client';

import { useIotaClientContext } from './useIotaClient.js';

export type NetworkConfig<T extends object = object> = IotaClientOptions & {
    variables?: T;
};

export function createNetworkConfig<
    const T extends Record<string, Config>,
    Config extends NetworkConfig<Variables> = T[keyof T],
    Variables extends object = NonNullable<Config['variables']>,
>(networkConfig: T) {
    function useNetworkConfig(): Config {
        const { config } = useIotaClientContext();

        if (!config) {
            throw new Error('No network config found');
        }

        return config as T[keyof T];
    }

    function useNetworkVariables(): Variables {
        const { variables } = useNetworkConfig();

        return (variables ?? {}) as Variables;
    }

    function useNetworkVariable<K extends keyof Variables>(name: K): Variables[K] {
        const variables = useNetworkVariables();

        return variables[name];
    }

    return {
        networkConfig,
        useNetworkConfig,
        useNetworkVariables,
        useNetworkVariable,
    };
}

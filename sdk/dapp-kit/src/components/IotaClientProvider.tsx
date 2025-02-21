// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getNetwork, isIotaClient, Network, IotaClient } from '@iota/iota-sdk/client';
import type { IotaClientOptions } from '@iota/iota-sdk/client';
import { createContext, useMemo, useState } from 'react';

import type { NetworkConfig } from '../hooks/networkConfig.js';

type NetworkConfigs<T extends NetworkConfig | IotaClient = NetworkConfig | IotaClient> = Record<
    string,
    T
>;

export interface IotaClientProviderContext {
    client: IotaClient;
    networks: NetworkConfigs;
    network: string;
    config: NetworkConfig | null;
    selectNetwork: (network: string) => void;
}

export const IotaClientContext = createContext<IotaClientProviderContext | null>(null);

export type IotaClientProviderProps<T extends NetworkConfigs> = {
    createClient?: (name: keyof T, config: T[keyof T]) => IotaClient;
    children: React.ReactNode;
    networks?: T;
    onNetworkChange?: (network: keyof T & string) => void;
} & (
    | {
          defaultNetwork?: keyof T & string;
          network?: never;
      }
    | {
          defaultNetwork?: never;
          network?: keyof T & string;
      }
);

const DEFAULT_NETWORKS = {
    localnet: { url: getNetwork(Network.Localnet)?.url },
};

const DEFAULT_CREATE_CLIENT = function createClient(
    _name: string,
    config: NetworkConfig | IotaClient,
) {
    if (isIotaClient(config)) {
        return config;
    }

    return new IotaClient(config);
};

export function IotaClientProvider<T extends NetworkConfigs>(props: IotaClientProviderProps<T>) {
    const { onNetworkChange, network, children } = props;
    const networks = (props.networks ?? DEFAULT_NETWORKS) as T;
    const createClient =
        (props.createClient as typeof DEFAULT_CREATE_CLIENT) ?? DEFAULT_CREATE_CLIENT;

    const [selectedNetwork, setSelectedNetwork] = useState<keyof T & string>(
        props.network ?? props.defaultNetwork ?? (Object.keys(networks)[0] as keyof T & string),
    );

    const currentNetwork = props.network ?? selectedNetwork;

    const client = useMemo(() => {
        return createClient(currentNetwork, networks[currentNetwork]);
    }, [createClient, currentNetwork, networks]);

    const ctx = useMemo((): IotaClientProviderContext => {
        return {
            client,
            networks,
            network: currentNetwork,
            config:
                networks[currentNetwork] instanceof IotaClient
                    ? null
                    : (networks[currentNetwork] as IotaClientOptions),
            selectNetwork: (newNetwork) => {
                if (currentNetwork === newNetwork) {
                    return;
                }

                if (!network && newNetwork !== selectedNetwork) {
                    setSelectedNetwork(newNetwork);
                }

                onNetworkChange?.(newNetwork);
            },
        };
    }, [client, networks, selectedNetwork, currentNetwork, network, onNetworkChange]);

    return <IotaClientContext.Provider value={ctx}>{children}</IotaClientContext.Provider>;
}

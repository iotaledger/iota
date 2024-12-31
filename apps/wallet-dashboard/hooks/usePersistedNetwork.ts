// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientContext } from '@iota/dapp-kit';
import { NetworkConfiguration } from '@iota/iota-sdk/client';
import { useLocalStorage } from '@iota/core';
import toast from 'react-hot-toast';

export function usePersistedNetwork() {
    const clientContext = useIotaClientContext();
    const activeNetwork = clientContext.network;

    const [persistedNetwork, setPersistedNetwork] = useLocalStorage<string>(
        'network_iota-dashboard',
        activeNetwork,
    );

    async function handleNetworkChange(network: NetworkConfiguration) {
        if (persistedNetwork === network.id) {
            return;
        }

        clientContext.selectNetwork(network.id);
        setPersistedNetwork(network.id);
        toast.success(`Switched to ${network.name}`);
    }

    return {
        persistedNetwork,
        handleNetworkChange,
    };
}

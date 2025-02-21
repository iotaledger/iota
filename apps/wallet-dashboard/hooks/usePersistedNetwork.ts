// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientContext } from '@iota/dapp-kit';
import { NetworkConfiguration } from '@iota/iota-sdk/client';
import { useLocalStorage } from '@iota/core';
import toast from 'react-hot-toast';
import { useEffect } from 'react';
import { ampli } from '@/lib/utils/analytics';

export function usePersistedNetwork() {
    const clientContext = useIotaClientContext();
    const activeNetwork = clientContext.network;

    const LOCAL_STORAGE_KEY = 'network_iota-dashboard';

    const [persistedNetwork, setPersistedNetwork] = useLocalStorage<string>(
        LOCAL_STORAGE_KEY,
        activeNetwork,
    );

    async function handleNetworkChange(network: NetworkConfiguration) {
        if (persistedNetwork === network.id) {
            return;
        }

        clientContext.selectNetwork(network.id);
        setPersistedNetwork(network.id);
        toast.success(`Switched to ${network.name}`);
        ampli.switchedNetwork({
            toNetwork: network.name,
        });
    }

    useEffect(() => {
        if (activeNetwork !== persistedNetwork) {
            setPersistedNetwork(activeNetwork);
        }
    }, [persistedNetwork, activeNetwork, setPersistedNetwork]);

    return {
        persistedNetwork,
        handleNetworkChange,
    };
}

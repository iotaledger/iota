// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useIotaClient } from '@iota/dapp-kit';
import { getFullnodeUrl, IotaClient, Network } from '@iota/iota-sdk/client';
import { useMemo } from 'react';

import { useNetwork } from '~/hooks';

// TODO: Use enhanced RPC locally by default
export function useEnhancedRpcClient(): IotaClient {
    const [network] = useNetwork();
    const client = useIotaClient();
    const enhancedRpc = useMemo(() => {
        if (network === Network.Localnet) {
            return new IotaClient({ url: getFullnodeUrl(Network.Localnet) });
        }

        return client;
    }, [network, client]);

    return enhancedRpc;
}

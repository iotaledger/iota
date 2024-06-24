// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useIotaClient } from '@iota/dapp-kit';
import { IotaClient } from '@iota/iota.js/client';
import { useMemo } from 'react';

import { useNetwork } from '~/context';
import { Network } from '~/utils/api/defaultRpcClient';

// TODO: Use enhanced RPC locally by default
export function useEnhancedRpcClient(): IotaClient {
    const [network] = useNetwork();
    const client = useIotaClient();
    const enhancedRpc = useMemo(() => {
        if (network === Network.Local) {
            return new IotaClient({ url: 'http://localhost:9124' });
        }

        return client;
    }, [network, client]);

    return enhancedRpc;
}

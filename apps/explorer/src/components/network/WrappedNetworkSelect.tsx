// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { useContext } from 'react';

import { NetworkContext } from '~/contexts';
import { NetworkSelect, type NetworkOption } from '~/components/ui';
import { ampli } from '~/lib/utils';
import { getAllNetworks } from '@iota/iota-sdk/client';

export function NetworkSelector(): JSX.Element {
    const [network, setNetwork] = useContext(NetworkContext);
    const { data } = useIotaClientQuery('getLatestIotaSystemState');
    const { data: binaryVersion } = useIotaClientQuery('getRpcApiVersion');

    const networks = Object.values(getAllNetworks()).map((network) => ({
        id: network.id,
        label: network.name,
    })) as NetworkOption[];

    return (
        <NetworkSelect
            value={network}
            onChange={(networkId) => {
                ampli.switchedNetwork({ toNetwork: networkId });
                setNetwork(networkId);
            }}
            networks={networks}
            version={data?.protocolVersion}
            binaryVersion={binaryVersion}
        />
    );
}

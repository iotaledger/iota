// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from '@iota/dapp-kit';
import { useContext } from 'react';

import { NetworkContext } from '../../context';
import { NetworkSelect, type NetworkOption } from '~/ui/header/NetworkSelect';
import { ampli } from '~/utils/analytics/ampli';
import { getAllNetworks } from '@iota/iota.js/client';

export default function WrappedNetworkSelect() {
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

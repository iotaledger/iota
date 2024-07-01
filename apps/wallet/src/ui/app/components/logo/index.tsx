// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaCustomRpc, IotaDevnet, IotaLocal, IotaMainnet, IotaTestnet } from '@iota/icons';
import { Network } from '@iota/iota.js/client';

interface LogoProps {
    network?: Network;
}

const NETWORK_LOGOS = {
    [Network.Mainnet]: IotaMainnet,
    [Network.Devnet]: IotaDevnet,
    [Network.Testnet]: IotaTestnet,
    [Network.Local]: IotaLocal,
    [Network.Custom]: IotaCustomRpc,
};

const Logo = ({ network }: LogoProps) => {
    let LogoComponent = NETWORK_LOGOS[Network.Custom];

    if (network && NETWORK_LOGOS[network]) {
        LogoComponent = NETWORK_LOGOS[network];
    }

    return <LogoComponent className="h-7 w-walletLogo text-gray-90" />;
};

export default Logo;

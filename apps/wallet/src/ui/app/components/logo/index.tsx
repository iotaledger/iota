// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { API_ENV } from '_src/shared/api-env';
import { IotaCustomRpc, IotaDevnet, IotaLocal, IotaMainnet, IotaTestnet } from '@iota/icons';

type LogoProps = {
    networkName?: API_ENV;
};

const networkLogos = {
    [API_ENV.mainnet]: IotaMainnet,
    [API_ENV.devNet]: IotaDevnet,
    [API_ENV.testNet]: IotaTestnet,
    [API_ENV.local]: IotaLocal,
    [API_ENV.customRPC]: IotaCustomRpc,
};

const Logo = ({ networkName }: LogoProps) => {
    const LogoComponent = networkName ? networkLogos[networkName] : networkLogos[API_ENV.mainnet];

    return <LogoComponent className="h-7 w-walletLogo text-gray-90" />;
};

export default Logo;

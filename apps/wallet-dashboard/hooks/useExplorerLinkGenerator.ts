// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientContext } from '@iota/dapp-kit';
import { getNetwork } from '@iota/iota-sdk/client';

export function useExplorerLinkGenerator(): (path: string) => string {
    const { network } = useIotaClientContext();
    const networkConfig = getNetwork(network);

    return (path) => `${networkConfig.explorer}/${path}`;
}

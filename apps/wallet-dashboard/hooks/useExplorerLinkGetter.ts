// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { getExplorerLink } from '@iota/core';
import { useCurrentAccount, useIotaClientContext } from '@iota/dapp-kit';

export function useExplorerLinkGetter(): (
    linkConfig: Parameters<typeof getExplorerLink>[0],
) => ReturnType<typeof getExplorerLink> {
    const { network } = useIotaClientContext();
    const address = useCurrentAccount()?.address || null;

    return (linkConfig) => getExplorerLink(linkConfig, address, network);
}

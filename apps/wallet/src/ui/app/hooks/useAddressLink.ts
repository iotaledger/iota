// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useExplorerLink } from '_app/hooks/useExplorerLink';
import { ExplorerLinkType } from '_components';
import { formatAddress } from '@iota/iota-sdk/utils';
import { useResolveIotaNSName } from '@iota/dapp-kit';
import { isIotaNSName } from '@iota/core';

export function useAddressLink(inputAddress: string | null) {
    const { data: domainName } = useResolveIotaNSName(inputAddress);
    const outputAddress = domainName ?? (inputAddress || '');
    const explorerHref = useExplorerLink({
        type: ExplorerLinkType.Address,
        address: outputAddress || undefined,
    });

    return {
        explorerHref: explorerHref || '',
        addressFull: inputAddress || '',
        address: isIotaNSName(outputAddress) ? outputAddress : formatAddress(outputAddress),
    };
}

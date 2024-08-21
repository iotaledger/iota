// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useExplorerLink } from '_app/hooks/useExplorerLink';
import { ExplorerLinkType } from '_components';
import { formatAddress } from '@iota/iota-sdk/utils';

export function useAddressLink(inputAddress: string | null) {
    const outputAddress = inputAddress || '';
    const explorerHref = useExplorerLink({
        type: ExplorerLinkType.Address,
        address: outputAddress || undefined,
    });

    return {
        explorerHref: explorerHref || '',
        addressFull: inputAddress || '',
        address: formatAddress(outputAddress),
    };
}

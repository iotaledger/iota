// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import ExplorerLink from '_components/explorer-link';
import { ExplorerLinkType } from '_components/explorer-link/ExplorerLinkType';
import { isIotaNSName } from '@iota/core';
import { formatAddress } from '@iota/iota.js/utils';

interface TxnAddressLinkProps {
    address: string;
}

export function TxnAddressLink({ address }: TxnAddressLinkProps) {
    return (
        <ExplorerLink
            type={ExplorerLinkType.Address}
            address={address}
            title="View on Iota Explorer"
            showIcon={false}
        >
            {isIotaNSName(address) ? address : formatAddress(address)}
        </ExplorerLink>
    );
}

// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink, ExplorerLinkType } from '_components';
import { formatAddress } from '@iota/iota-sdk/utils';

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
            {formatAddress(address)}
        </ExplorerLink>
    );
}

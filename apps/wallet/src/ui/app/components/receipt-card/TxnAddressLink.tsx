// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import ExplorerLink from '_components/explorer-link';
import { ExplorerLinkType } from '_components/explorer-link/ExplorerLinkType';
import { formatAddress, isValidIotaNSName } from '@iota/iota-sdk/utils';

type TxnAddressLinkProps = {
    address: string;
};

export function TxnAddressLink({ address }: TxnAddressLinkProps) {
    return (
        <ExplorerLink
            type={ExplorerLinkType.address}
            address={address}
            title="View on Iota Explorer"
            showIcon={false}
        >
            {isValidIotaNSName(address) ? address : formatAddress(address)}
        </ExplorerLink>
    );
}

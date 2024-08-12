// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import ExplorerLink from '_src/ui/app/components/explorer-link';
import { ExplorerLinkType } from '_src/ui/app/components/explorer-link/ExplorerLinkType';
import { useActiveAddress } from '_src/ui/app/hooks';
import { getOwnerDisplay } from '@iota/core';

import { Text } from '_app/shared/text';
import { SummaryCardFooter } from './Card';

export function OwnerFooter({ owner, ownerType }: { owner?: string; ownerType?: string }) {
    const address = useActiveAddress();
    const { ownerDisplay, isOwner } = getOwnerDisplay(owner, ownerType, address);

    return (
        <SummaryCardFooter>
            <Text variant="pBody" weight="medium" color="steel-dark">
                Owner
            </Text>
            <div className="flex justify-end">
                {isOwner ? (
                    <Text variant="body" weight="medium" color="hero-dark">
                        {ownerDisplay}
                    </Text>
                ) : (
                    <ExplorerLink
                        type={ExplorerLinkType.Address}
                        title={owner}
                        address={owner}
                        className="text-hero-dark font-mono text-body font-medium no-underline"
                    >
                        {ownerDisplay}
                    </ExplorerLink>
                )}
            </div>
        </SummaryCardFooter>
    );
}

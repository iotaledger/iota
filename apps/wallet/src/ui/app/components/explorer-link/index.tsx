// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExternalLink } from '_components';
import { ArrowUpRight16 } from '@iota/icons';
import { formatAddress } from '@iota/iota-sdk/utils';
import type { ReactNode } from 'react';

import { useExplorerLink, type ExplorerLinkConfig } from '../../hooks/useExplorerLink';
import { Text } from '../../shared/text';
import st from './ExplorerLink.module.scss';
import { ExplorerLinkType } from './ExplorerLinkType';

export type ExplorerLinkProps = ExplorerLinkConfig & {
    track?: boolean;
    children?: ReactNode;
    className?: string;
    title?: string;
    showIcon?: boolean;
};

export function ExplorerLink({
    track,
    children,
    className,
    title,
    showIcon,
    ...linkConfig
}: ExplorerLinkProps) {
    const explorerHref = useExplorerLink(linkConfig);
    if (!explorerHref) {
        return null;
    }

    return (
        <ExternalLink href={explorerHref} className={className} title={title}>
            <>
                {children} {showIcon && <ArrowUpRight16 className={st.explorerIcon} />}
            </>
        </ExternalLink>
    );
}

export function AddressLink({ address }: { address: string }) {
    return (
        <ExplorerLink
            type={ExplorerLinkType.Address}
            address={address}
            className="text-hero-dark inline-block no-underline"
        >
            <Text variant="subtitle" weight="semibold" truncate mono>
                {formatAddress(address)}
            </Text>
        </ExplorerLink>
    );
}

export * from './ExplorerLinkType';

export default ExplorerLink;

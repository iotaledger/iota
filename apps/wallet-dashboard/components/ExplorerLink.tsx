// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useExplorerLinkGetter } from '@/hooks';
import { getExplorerLink } from '@iota/core';
import { ExternalLink } from './ExternalLink';

type GetExplorerLinkArgs = Parameters<typeof getExplorerLink>[0];

type ExplorerLinkProps = GetExplorerLinkArgs & {
    isExternal?: boolean;
};

export function ExplorerLink({
    children,
    isExternal,
    ...getLinkProps
}: React.PropsWithChildren<ExplorerLinkProps>): React.JSX.Element {
    const getExplorerLink = useExplorerLinkGetter();
    const href = getExplorerLink(getLinkProps) ?? '#';

    return (
        <ExternalLink href={href} type={getLinkProps.type} trackEvent={href !== '#'}>
            {children}
        </ExternalLink>
    );
}

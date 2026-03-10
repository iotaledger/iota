// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useExplorerLinkGetter } from '@/hooks';
import { getExplorerLink } from '@iota/core';
import Link from 'next/link';
import { ampli } from '@/lib/utils/analytics/ampli';

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

    function handleClick() {
        if (getLinkProps.type) {
            ampli.externalLinkOpened({ type: getLinkProps.type });
        }
    }

    return (
        <Link href={href} target="_blank" rel="noopener noreferrer" onClick={handleClick}>
            {children}
        </Link>
    );
}

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useExplorerLinkGetter } from '@/hooks';
import { getExplorerLink } from '@iota/core';
import Link from 'next/link';

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
        <Link href={href} target="_blank" rel="noopener noreferrer">
            {children}
        </Link>
    );
}

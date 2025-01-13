// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExternalLink } from '_components';
import type { ReactNode } from 'react';
import { type ExplorerLinkConfig, ExplorerLinkType } from '@iota/core';
import { useExplorerLink } from '_hooks';
import st from './ExplorerLink.module.scss';
import clsx from 'clsx';
import { ArrowTopRight } from '@iota/ui-icons';

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
        <ExternalLink
            href={explorerHref}
            className={clsx('text-body-md text-primary-30 dark:text-primary-80', className)}
            title={title}
        >
            <>
                {children} {showIcon && <ArrowTopRight className={st.explorerIcon} />}
            </>
        </ExternalLink>
    );
}

export { ExplorerLinkType };

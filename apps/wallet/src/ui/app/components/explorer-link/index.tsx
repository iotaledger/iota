// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExternalLink } from '_components';
import type { ReactNode } from 'react';
import { type ExplorerLinkConfig, ExplorerLinkType } from '@iota/core';
import { useExplorerLink } from '_hooks';
import st from './ExplorerLink.module.scss';
import clsx from 'clsx';
import { ArrowTopRight } from '@iota/apps-ui-icons';

export type ExplorerLinkProps = ExplorerLinkConfig & {
    children?: ReactNode;
    className?: string;
    title?: string;
    showIcon?: boolean;
    eventType?: string;
};

export function ExplorerLink({
    children,
    className,
    title,
    showIcon,
    eventType,
    ...linkConfig
}: ExplorerLinkProps) {
    const explorerHref = useExplorerLink(linkConfig);
    if (!explorerHref) {
        return null;
    }

    return (
        <ExternalLink
            href={explorerHref}
            className={clsx(
                'text-body-md text-iota-primary-30 dark:text-iota-primary-80',
                className,
            )}
            title={title}
            type={eventType}
        >
            <>
                {children} {showIcon && <ArrowTopRight className={st.explorerIcon} />}
            </>
        </ExternalLink>
    );
}

export { ExplorerLinkType };

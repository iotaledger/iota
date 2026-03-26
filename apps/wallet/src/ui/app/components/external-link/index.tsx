// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from 'react';
import { ampli } from '_src/shared/analytics/ampli';

export interface ExternalLinkProps {
    href: string;
    className?: string;
    children: ReactNode;
    title?: string;
    onClick?(): void;
    type?: string; // e.g. 'documentation' | 'application' | 'address' | 'digest' | ...
    trackEvent?: boolean;
}

export function ExternalLink({
    href,
    className,
    children,
    title,
    onClick,
    type,
    trackEvent = true,
}: ExternalLinkProps) {
    const handleClick = () => {
        if (trackEvent && type) {
            ampli.openedLink({
                type,
            });
        }
        onClick?.();
    };

    return (
        <a
            href={href}
            target="_blank"
            className={className}
            rel="noreferrer noopener"
            title={title}
            onClick={handleClick}
        >
            {children}
        </a>
    );
}

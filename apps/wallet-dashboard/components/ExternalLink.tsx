// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import type { ReactNode } from 'react';
import Link from 'next/link';
import { ampli } from '@/lib/utils/analytics';

export interface ExternalLinkProps {
    href: string;
    className?: string;
    children: ReactNode;
    title?: string;
    onClick?(): void;
    type?: string; // e.g. 'documentation' | 'application' | 'address' | 'digest' | ...
    trackEvent?: boolean;
}

/**
 * External link component with analytics tracking.
 * For programmatic external link opening (e.g., window.open), use the useExternalLink hook instead.
 */
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
        <Link
            href={href}
            target="_blank"
            className={className}
            rel="noopener noreferrer"
            title={title}
            onClick={handleClick}
        >
            {children}
        </Link>
    );
}

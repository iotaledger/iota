// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from 'react';

export interface ExternalLinkProps {
    href: string;
    className?: string;
    children: ReactNode;
    title?: string;
    onClick?(): void;
}

export function ExternalLink({ href, className, children, title, onClick }: ExternalLinkProps) {
    return (
        <a
            href={href}
            target="_blank"
            className={className}
            rel="noreferrer noopener"
            title={title}
            onClick={onClick}
        >
            {children}
        </a>
    );
}

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { usePathname } from 'next/navigation';
import React from 'react';
import Link from 'next/link';

interface RouteLinkProps {
    title: string;
    path: string;
}

export function RouteLink({ title, path }: RouteLinkProps): JSX.Element {
    const currentPath = usePathname();

    const isActive = currentPath && (path === currentPath || path.startsWith(currentPath));

    return (
        <Link href={path}>
            <div
                className={`sidebar-item justify-between space-x-5 ${isActive ? 'underline' : ''}`}
            >
                <div className="flex items-center justify-between space-x-5">
                    <span className="flex-shrink-0 origin-left duration-300">{title}</span>
                </div>
            </div>
        </Link>
    );
}

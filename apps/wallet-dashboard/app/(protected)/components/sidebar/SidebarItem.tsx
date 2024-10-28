// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import type { ProtectedRoute } from '@/lib/interfaces';
import { NavbarItem } from '@iota/apps-ui-kit';
import { usePathname } from 'next/navigation';
import Link from 'next/link';

export function SidebarItem({ icon, path }: ProtectedRoute) {
    const pathname = usePathname();
    const RouteIcon = icon;
    const isActive = pathname === path;
    return (
        <Link href={path} className="px-sm py-xxs">
            <NavbarItem isSelected={isActive} icon={<RouteIcon />} />
        </Link>
    );
}

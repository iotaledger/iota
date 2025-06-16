// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import type { ProtectedRoute } from '@/lib/interfaces';
import { NavbarItem, Tooltip, TooltipPosition } from '@iota/apps-ui-kit';
import { usePathname } from 'next/navigation';
import Link from 'next/link';

export function SidebarItem({ icon, title, path, id }: ProtectedRoute) {
    const pathname = usePathname();
    const RouteIcon = icon;
    const isActive = pathname === path;
    return (
        <Tooltip text={title} position={TooltipPosition.Right}>
            <Link href={path} className="relative px-sm py-xxs" data-testid={`sidebar-${id}`}>
                <NavbarItem isSelected={isActive} icon={<RouteIcon />} />
            </Link>
        </Tooltip>
    );
}

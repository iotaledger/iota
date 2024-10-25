// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { PROTECTED_ROUTES } from '@/lib/constants';
import { NavbarItem } from '@iota/apps-ui-kit';
import { IotaLogoMark } from '@iota/ui-icons';
import { usePathname } from 'next/navigation';
import Link from 'next/link';

export function Sidebar() {
    const pathname = usePathname();
    return (
        <nav className="fixed left-0 top-0 flex h-screen flex-col items-center gap-y-2xl bg-neutral-100 py-xl">
            <IotaLogoMark className="h-10 w-10 text-neutral-10 dark:text-neutral-92" />
            <div className="flex flex-col gap-y-xs">
                {PROTECTED_ROUTES.map(({ title, path, icon }) => {
                    const RouteIcon = icon;
                    const isActive = pathname === path;
                    return (
                        <Link key={title} href={path} className="px-sm py-xxs">
                            <NavbarItem isSelected={isActive} icon={<RouteIcon />} />
                        </Link>
                    );
                })}
            </div>
        </nav>
    );
}

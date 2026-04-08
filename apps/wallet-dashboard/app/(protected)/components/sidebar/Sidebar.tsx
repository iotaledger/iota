// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PROTECTED_ROUTES } from '@/lib/constants/routes.constants';
import { IotaLogoMark } from '@iota/apps-ui-icons';
import { SidebarItem } from './SidebarItem';

export function Sidebar() {
    return (
        <nav
            data-testid="sidebar"
            className="flex h-screen flex-col items-center gap-y-2xl bg-iota-neutral-100 py-xl dark:bg-iota-neutral-6"
        >
            <IotaLogoMark className="h-10 w-10 text-iota-neutral-10 dark:text-iota-neutral-92" />
            <div className="flex flex-col gap-y-xs">
                {PROTECTED_ROUTES.map((route) => (
                    <SidebarItem key={route.path} {...route} />
                ))}
            </div>
        </nav>
    );
}

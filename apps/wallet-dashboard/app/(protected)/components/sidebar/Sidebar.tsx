// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PROTECTED_ROUTES } from '@/lib/constants/routes.constants';
import { IotaLogoMark } from '@iota/apps-ui-icons';
import { SidebarItem } from './SidebarItem';
import { Feature } from '@iota/core';
import { useFeature } from '@growthbook/growthbook-react';
import { ProtectedRouteTitle } from '@/lib/enums';

export function Sidebar() {
    const featureFlags = {
        [ProtectedRouteTitle.Migration]: useFeature<boolean>(Feature.StardustMigration).value,
        [ProtectedRouteTitle.Vesting]: useFeature<boolean>(Feature.SupplyIncreaseVesting).value,
    };

    const filteredRoutes = PROTECTED_ROUTES.filter(({ title }) => {
        return title in featureFlags ? featureFlags[title as keyof typeof featureFlags] : true;
    });

    return (
        <nav className="flex h-screen flex-col items-center gap-y-2xl bg-neutral-100 py-xl dark:bg-neutral-6">
            <IotaLogoMark className="h-10 w-10 text-neutral-10 dark:text-neutral-92" />
            <div className="flex flex-col gap-y-xs">
                {filteredRoutes.map((route) => (
                    <SidebarItem key={route.path} {...route} />
                ))}
            </div>
        </nav>
    );
}

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ProtectedAppRoute } from '@/lib/enums';
import { Activity, Assets, Calendar, Home, Migration, Tokens } from '@iota/ui-icons';
import { AppRoute } from '@/lib/interfaces';

export const HOMEPAGE_ROUTE: AppRoute = {
    title: ProtectedAppRoute.Home,
    path: '/home',
    icon: Home,
};

export const ASSETS_ROUTE: AppRoute = {
    title: ProtectedAppRoute.Assets,
    path: '/assets',
    icon: Assets,
};

export const STAKING_ROUTE: AppRoute = {
    title: ProtectedAppRoute.Staking,
    path: '/staking',
    icon: Activity,
};

export const ACTIVITY_ROUTE: AppRoute = {
    title: ProtectedAppRoute.Activity,
    path: '/activity',
    icon: Tokens,
};
export const MIGRATIONS_ROUTE: AppRoute = {
    title: ProtectedAppRoute.Migrations,
    path: '/migrations',
    icon: Calendar,
};
export const VESTING_ROUTE: AppRoute = {
    title: ProtectedAppRoute.Vesting,
    path: '/vesting',
    icon: Migration,
};

export const PROTECTED_ROUTES = [
    HOMEPAGE_ROUTE,
    ASSETS_ROUTE,
    STAKING_ROUTE,
    ACTIVITY_ROUTE,
    MIGRATIONS_ROUTE,
    VESTING_ROUTE,
] as const satisfies AppRoute[];

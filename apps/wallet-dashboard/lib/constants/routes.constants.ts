// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ProtectedAppRoute } from '@/lib/enums';
import { Activity, Assets, Calendar, Home, Migration, Tokens } from '@iota/ui-icons';

export const HOMEPAGE_ROUTE = { title: ProtectedAppRoute.Home, path: '/home', icon: Home };

export const ASSETS_ROUTE = { title: ProtectedAppRoute.Assets, path: '/assets', icon: Assets };

export const STAKING_ROUTE = { title: ProtectedAppRoute.Staking, path: '/staking', icon: Activity };

export const ACTIVITY_ROUTE = {
    title: ProtectedAppRoute.Activity,
    path: '/activity',
    icon: Tokens,
};
export const MIGRATIONS_ROUTE = {
    title: ProtectedAppRoute.Migrations,
    path: '/migrations',
    icon: Calendar,
};
export const VESTING_ROUTE = {
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
] as const;

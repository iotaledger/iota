// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ProtectedRoute } from '../interfaces';
import { ProtectedRouteTitle } from '../enums';
import { Activity, Assets, Home, Migration, Stake, Vesting } from '@iota/apps-ui-icons';

export const HOMEPAGE_ROUTE: ProtectedRoute = {
    title: ProtectedRouteTitle.Home,
    path: '/home',
    icon: Home,
};

export const ASSETS_ROUTE: ProtectedRoute = {
    title: ProtectedRouteTitle.Assets,
    path: '/assets',
    icon: Assets,
};

export const STAKING_ROUTE: ProtectedRoute = {
    title: ProtectedRouteTitle.Staking,
    path: '/staking',
    icon: Stake,
};

export const ACTIVITY_ROUTE: ProtectedRoute = {
    title: ProtectedRouteTitle.Activity,
    path: '/activity',
    icon: Activity,
};
export const MIGRATION_ROUTE: ProtectedRoute = {
    title: ProtectedRouteTitle.Migration,
    path: '/migration',
    icon: Migration,
};
export const VESTING_ROUTE: ProtectedRoute = {
    title: ProtectedRouteTitle.Vesting,
    path: '/vesting',
    icon: Vesting,
};

export const PROTECTED_ROUTES = [
    HOMEPAGE_ROUTE,
    ASSETS_ROUTE,
    STAKING_ROUTE,
    ACTIVITY_ROUTE,
    VESTING_ROUTE,
    MIGRATION_ROUTE,
] as const satisfies ProtectedRoute[];

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeature } from '@growthbook/growthbook-react';
import { Feature } from '@iota/core';
import { prepareLinkToCompare } from '_src/shared/utils';
import { useEffect, useMemo } from 'react';
import { permissionsSelectors } from '../redux/slices/permissions';
import { useAppSelector, useBackgroundClient } from '.';
import { type DAppEntry } from '../components/iota-apps/IotaApp';

/**
 * Hook to get all connected apps with their details
 *
 * @returns {object} Connected apps data and loading state
 */
export function useConnectedApps() {
    const backgroundClient = useBackgroundClient();

    // Fetch permissions when the component mounts
    useEffect(() => {
        backgroundClient.sendGetPermissionRequests();
    }, [backgroundClient]);

    const ecosystemApps = useFeature<DAppEntry[]>(Feature.WalletDapps).value ?? [];
    const loading = useAppSelector(({ permissions }) => !permissions.initialized);
    const allPermissions = useAppSelector(permissionsSelectors.selectAll);

    // Process permissions and match with ecosystem apps to get connected apps
    const connectedApps = useMemo(
        () =>
            allPermissions
                .filter(({ allowed }) => allowed)
                .map((aPermission) => {
                    const matchedEcosystemApp = ecosystemApps.find((anEcosystemApp) => {
                        const originAdj = prepareLinkToCompare(aPermission.origin);
                        const pageLinkAdj = aPermission.pagelink
                            ? prepareLinkToCompare(aPermission.pagelink)
                            : null;
                        const anEcosystemAppLinkAdj = prepareLinkToCompare(anEcosystemApp.link);
                        return (
                            originAdj === anEcosystemAppLinkAdj ||
                            pageLinkAdj === anEcosystemAppLinkAdj
                        );
                    });
                    let appNameFromOrigin = '';
                    try {
                        appNameFromOrigin = new URL(aPermission.origin).hostname
                            .replace('www.', '')
                            .split('.')[0];
                    } catch (e) {
                        // do nothing
                    }
                    return {
                        name: aPermission.name || appNameFromOrigin,
                        description: '',
                        icon: aPermission.favIcon || '',
                        link: aPermission.pagelink || aPermission.origin,
                        tags: [],
                        // override data from ecosystemApps
                        ...matchedEcosystemApp,
                        permissionID: aPermission.id,
                    };
                }),
        [allPermissions, ecosystemApps],
    );

    return {
        connectedApps,
        loading,
        ecosystemApps,
    };
}

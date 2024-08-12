// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Heading } from '_app/shared/heading';
import { Text } from '_app/shared/text';
import { useAppSelector } from '_hooks';
import { Feature } from '_src/shared/experimentation/features';
import { prepareLinkToCompare } from '_src/shared/utils';
import { useFeature } from '@growthbook/growthbook-react';
import { useEffect, useMemo } from 'react';

import { useBackgroundClient } from '../../hooks/useBackgroundClient';
import { permissionsSelectors } from '../../redux/slices/permissions';
import Loading from '_components/loading';
import { IotaApp, type DAppEntry } from './IotaApp';
import { IotaAppEmpty } from './IotaAppEmpty';

function ConnectedDapps() {
    const backgroundClient = useBackgroundClient();
    useEffect(() => {
        backgroundClient.sendGetPermissionRequests();
    }, [backgroundClient]);
    const ecosystemApps = useFeature<DAppEntry[]>(Feature.WalletDapps).value ?? [];
    const loading = useAppSelector(({ permissions }) => !permissions.initialized);
    const allPermissions = useAppSelector(permissionsSelectors.selectAll);
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
    return (
        <Loading loading={loading}>
            <div className="flex justify-center">
                <Heading variant="heading6" color="gray-90" weight="semibold">
                    Active Connections
                </Heading>
            </div>
            <div className="my-4">
                <Text variant="pBodySmall" color="gray-80" weight="normal">
                    Apps you have connected to through the Iota Wallet in this browser.
                </Text>
            </div>

            <div className="grid grid-cols-2 gap-3.75">
                {connectedApps.length ? (
                    connectedApps.map((app) => (
                        <IotaApp key={app.permissionID} {...app} displayType="card" />
                    ))
                ) : (
                    <>
                        <IotaAppEmpty displayType="card" />
                        <IotaAppEmpty displayType="card" />
                    </>
                )}
            </div>
        </Loading>
    );
}

export default ConnectedDapps;

// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header, Title, TitleSize } from '@iota/apps-ui-kit';
import { useAppSelector } from '_hooks';
import { Feature } from '_src/shared/experimentation/features';
import { prepareLinkToCompare } from '_src/shared/utils';
import { useFeature } from '@growthbook/growthbook-react';
import { useEffect, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';

import { useBackgroundClient } from '../../hooks/useBackgroundClient';
import { permissionsSelectors } from '../../redux/slices/permissions';
import { Loading, NoData } from '_components';
import { type DAppEntry, IotaApp } from './IotaApp';

function ConnectedDapps() {
    const navigate = useNavigate();

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

    function handleBack() {
        navigate('/');
    }

    return (
        <Loading loading={loading}>
            <>
                <Header title={'Apps'} titleCentered onBack={handleBack} />
                <div className="flex flex-1 flex-col gap-md p-md">
                    {connectedApps.length ? (
                        <div className="flex flex-col gap-xs">
                            <Title title="Active Connections" size={TitleSize.Small} />
                            {connectedApps.map((app) => (
                                <IotaApp key={app.permissionID} {...app} displayType="card" />
                            ))}
                        </div>
                    ) : (
                        <NoData message="No connected apps found." />
                    )}
                </div>
            </>
        </Loading>
    );
}

export default ConnectedDapps;

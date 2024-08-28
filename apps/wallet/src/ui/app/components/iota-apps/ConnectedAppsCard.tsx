// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ButtonUnstyled,
    Chip,
    Header,
    SegmentedButton,
    SegmentedButtonType,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import { useAppSelector } from '_hooks';
import { Feature } from '_src/shared/experimentation/features';
import { prepareLinkToCompare } from '_src/shared/utils';
import { useFeature } from '@growthbook/growthbook-react';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { useBackgroundClient } from '../../hooks/useBackgroundClient';
import { permissionsSelectors } from '../../redux/slices/permissions';
import { Loading } from '_components';
import { type DAppEntry, IotaApp } from './IotaApp';
import { IotaAppEmpty } from './IotaAppEmpty';

enum ConnectedDappCategory {
    All = 'all',
    DeFi = 'defi',
    Dex = 'dex',
    Connections = 'connections',
}

const CONNTECTED_DAPPS_CATEGORIES = [
    {
        label: 'All',
        value: ConnectedDappCategory.All,
    },
    {
        label: 'DeFi',
        value: ConnectedDappCategory.DeFi,
    },
    {
        label: 'Dex',
        value: ConnectedDappCategory.Dex,
    },
    {
        label: 'Connections',
        value: ConnectedDappCategory.Connections,
    },
];

function ConnectedDapps() {
    const navigate = useNavigate();
    const [selectedDappCategory, setSelectedDappCategory] = useState(ConnectedDappCategory.All);

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
                <div className="flex flex-col gap-md p-md">
                    <SegmentedButton type={SegmentedButtonType.Transparent}>
                        {CONNTECTED_DAPPS_CATEGORIES.map(({ label, value }) => (
                            <ButtonUnstyled onClick={() => setSelectedDappCategory(value)}>
                                <Chip label={label} selected={selectedDappCategory === value} />
                            </ButtonUnstyled>
                        ))}
                    </SegmentedButton>
                    {[ConnectedDappCategory.All, ConnectedDappCategory.Connections].includes(
                        selectedDappCategory,
                    ) && connectedApps.length ? (
                        <div className="flex flex-col gap-xs">
                            <Title title="Active Connections" size={TitleSize.Small} />
                            {connectedApps.map((app) => (
                                <IotaApp key={app.permissionID} {...app} displayType="card" />
                            ))}
                        </div>
                    ) : (
                        <>
                            <IotaAppEmpty displayType="card" />
                            <IotaAppEmpty displayType="card" />
                        </>
                    )}
                </div>
            </>
        </Loading>
    );
}

export default ConnectedDapps;

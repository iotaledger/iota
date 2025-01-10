// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppSelector } from '_hooks';
import { Feature } from '@iota/core';
import { prepareLinkToCompare } from '_src/shared/utils';
import { useFeature } from '@growthbook/growthbook-react';
import { useMemo } from 'react';
import { useParams } from 'react-router-dom';
import { permissionsSelectors } from '../../redux/slices/permissions';
import { AppsPageBanner } from './Banner';
import { IotaApp, type DAppEntry } from './IotaApp';
import { IotaAppEmpty } from './IotaAppEmpty';
import { InfoBox, InfoBoxStyle, InfoBoxType, Header } from '@iota/apps-ui-kit';
import { Info } from '@iota/ui-icons';

export function AppsPlayGround() {
    const ecosystemApps = useFeature<DAppEntry[]>(Feature.WalletDapps).value;
    const { tagName } = useParams();

    const filteredEcosystemApps = useMemo(() => {
        if (!ecosystemApps) {
            return [];
        } else if (tagName) {
            return ecosystemApps.filter((app) => app.tags.includes(tagName));
        }
        return ecosystemApps;
    }, [ecosystemApps, tagName]);

    const allPermissions = useAppSelector(permissionsSelectors.selectAll);
    const linkToPermissionID = useMemo(() => {
        const map = new Map<string, string>();
        for (const aPermission of allPermissions) {
            map.set(prepareLinkToCompare(aPermission.origin), aPermission.id);
            if (aPermission.pagelink) {
                map.set(prepareLinkToCompare(aPermission.pagelink), aPermission.id);
            }
        }
        return map;
    }, [allPermissions]);

    return (
        <>
            <Header titleCentered title="IOTA Apps" />
            <AppsPageBanner />

            {filteredEcosystemApps?.length ? (
                <InfoBox
                    type={InfoBoxType.Default}
                    icon={<Info />}
                    style={InfoBoxStyle.Elevated}
                    supportingText="Apps below are actively curated but do not indicate any endorsement or
                        relationship with IOTA Wallet. Please DYOR."
                />
            ) : null}

            {filteredEcosystemApps?.length ? (
                <div className="mt-md flex flex-col gap-sm">
                    {filteredEcosystemApps.map((app) => (
                        <IotaApp
                            key={app.link}
                            {...app}
                            permissionID={linkToPermissionID.get(prepareLinkToCompare(app.link))}
                            displayType="full"
                            openAppSite
                        />
                    ))}
                </div>
            ) : (
                <IotaAppEmpty displayType="full" />
            )}
        </>
    );
}

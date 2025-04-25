// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppSelector, useConnectedApps } from '_hooks';
import { Feature } from '@iota/core';
import { prepareLinkToCompare } from '_src/shared/utils';
import { useFeature } from '@growthbook/growthbook-react';
import { useMemo } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import cx from 'clsx';
import { permissionsSelectors } from '../../redux/slices/permissions';
import { AppsPageBanner } from './Banner';
import { IotaApp, type DAppEntry } from './IotaApp';
import {
    Card,
    CardBody,
    CardImage,
    CardType,
    CardAction,
    CardActionType,
    ImageShape,
    ImageType,
} from '@iota/apps-ui-kit';
import { Vest, ArrowRight } from '@iota/apps-ui-icons';
import { PageTemplate } from '../PageTemplate';
import { NoData } from '../NoData';

export function AppsPlayGround() {
    const { connectedApps } = useConnectedApps();

    const navigate = useNavigate();

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
        <PageTemplate title="IOTA Apps" isTitleCentered>
            <div
                className={cx('flex flex-1 flex-col gap-md', {
                    'h-full items-center': !filteredEcosystemApps?.length,
                })}
            >
                {connectedApps?.length ? (
                    <Card
                        type={CardType.Filled}
                        onClick={() => {
                            navigate('/apps/connected');
                        }}
                    >
                        <CardImage shape={ImageShape.SquareRounded} type={ImageType.BgWhite}>
                            <Vest className="h-4 w-4 text-neutral-10 dark:text-white" />
                        </CardImage>
                        <CardBody
                            isTextTruncated
                            title="Active Connections"
                            subtitle="Manage Active Connections"
                        />
                        <CardAction
                            type={CardActionType.Link}
                            icon={<ArrowRight className="h-4 w-4" />}
                        />
                    </Card>
                ) : null}

                <AppsPageBanner />

                {/* Note: add when we'll add external dApps */}
                {/* {filteredEcosystemApps?.length ? (
                    <InfoBox
                        type={InfoBoxType.Warning}
                        icon={<Warning />}
                        style={InfoBoxStyle.Elevated}
                        supportingText="Apps below are actively curated but do not indicate any endorsement or
                        relationship with IOTA Wallet. Please DYOR."
                    />
                ) : null} */}

                {filteredEcosystemApps?.length ? (
                    <div className="flex flex-col gap-xs">
                        {filteredEcosystemApps.map((app) => (
                            <IotaApp
                                key={app.link}
                                {...app}
                                permissionID={linkToPermissionID.get(
                                    prepareLinkToCompare(app.link),
                                )}
                                displayType="card"
                                openAppSite
                            />
                        ))}
                    </div>
                ) : (
                    <NoData message="No apps found." />
                )}
            </div>
        </PageTemplate>
    );
}
